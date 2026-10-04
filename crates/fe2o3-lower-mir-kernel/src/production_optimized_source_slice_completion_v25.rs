#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceSliceArgumentCompletionV25 {
    parameter: SliceDefinition,
    ty: SemanticTypeIdV1,
    identity: fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1,
    scalar: ScalarType,
    exclusive: bool,
    reads: usize,
    writes: usize,
    axis: Option<fe2o3_kernel_ir::Axis>,
    different_projection: bool,
}

impl SourceSliceArgumentCompletionV25 {
    fn add(
        &mut self,
        entry: &SourceSliceEntryRegionV25<'_, '_>,
        projection: Option<fe2o3_kernel_ir::Axis>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        budget.charge_work(21)?;
        if self.ty != entry.abi.ty()
            || self.parameter != entry.optimized_parameter()
            || self.identity != entry.abi.identity()
            || self.scalar != entry.abi.scalar()
            || self.exclusive != entry.abi.is_exclusive_contract()
        {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "slice argument runtime contract differs",
            ));
        }
        self.record_projection_v25(entry.access.pair.output.writing, projection)
    }

    fn record_projection_v25(
        &mut self,
        writing: bool,
        projection: Option<fe2o3_kernel_ir::Axis>,
    ) -> SourceOwnedResultV18<()> {
        let first = self.reads == 0 && self.writes == 0;
        if first {
            self.axis = projection;
        }
        self.different_projection |= projection.is_none() || self.axis != projection;
        if writing {
            if !self.exclusive || projection.is_none() || self.different_projection {
                return Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "slice Store pairwise conflict remains unproved",
                ));
            }
            self.writes = self
                .writes
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        } else {
            if self.writes != 0 && self.different_projection {
                return Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "slice read/Store pairwise conflict remains unproved",
                ));
            }
            self.reads = self
                .reads
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
        Ok(())
    }
}

#[cfg(test)]
include!("production_optimized_source_slice_completion_commands_v25_tests.rs");

// Only the complete root census constructs this conjunction. It retains the
// genuine source/native scopes and all runtime requirements; it is not a launch
// grant and cannot be detached into a caller-authored allocation certificate.
struct SourceSliceRootCompletionV25<'s, 'a, 'g> {
    source: &'s PendingSharedEntryRegionsV18<'s, 'a>,
    native: &'s fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'s, 'g>,
    arguments: &'s [Option<SourceSliceArgumentCompletionV25>],
    launch: fe2o3_kernel_ir::ExplicitLaunchExtent,
    width: fe2o3_kernel_ir::FormalIndexWidth,
}

fn slice_completion_read_error_v25(
    error: PendingGlobalReadConditionErrorV18,
) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        PendingGlobalReadConditionErrorV18::Source(error)
        | PendingGlobalReadConditionErrorV18::Formal {
            source_refusal: error,
            ..
        }
        | PendingGlobalReadConditionErrorV18::Unproved {
            source_refusal: error,
            ..
        } => error,
        PendingGlobalReadConditionErrorV18::Native(error) => slice_entry_native_error_v25(error),
    }
}

fn slice_completion_native_error_v25(
    error: PendingGlobalNativeErrorV18,
) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        PendingGlobalNativeErrorV18::Source(error) => error,
        PendingGlobalNativeErrorV18::Native(error) => slice_entry_native_error_v25(error),
    }
}

fn slice_completion_add_v25(
    rows: &mut [Option<SourceSliceArgumentCompletionV25>],
    entry: &SourceSliceEntryRegionV25<'_, '_>,
    projection: Option<fe2o3_kernel_ir::Axis>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    budget.charge_work(8)?;
    let ordinal =
        usize::try_from(entry.original_argument()).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    let slot = rows
        .get_mut(ordinal)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "slice runtime argument ordinal differs",
        ))?;
    let row = slot
        .as_mut()
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "slice access has no complete parameter census row",
        ))?;
    row.add(entry, projection, budget)
}

include!("production_optimized_source_global_completion_v26.rs");
include!("production_optimized_source_slice_parameters_v26.rs");

impl PendingSharedEntryRegionsV18<'_, '_> {
    fn with_complete_slice_domains_v25<'work, F>(
        &self,
        native: &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>,
        reads: &GlobalReadFactsV18<'_, '_>,
        stores: &GlobalStoreFactsV25<'_, '_>,
        launch: fe2o3_kernel_ir::ExplicitLaunchExtent,
        width: fe2o3_kernel_ir::FormalIndexWidth,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: &mut F,
    ) -> SourceOwnedResultV18<()>
    where
        F: for<'s, 'a, 'g> FnMut(
            &SourceSliceRootCompletionV25<'s, 'a, 'g>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<()>,
    {
        self.with_complete_slice_domains_profile_v89::<false, F>(
            native, reads, stores, launch, width, budget, consume,
        )
    }

    fn with_complete_slice_domains_profile_v89<'work, const PREDICATED: bool, F>(
        &self,
        native: &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>,
        reads: &GlobalReadFactsV18<'_, '_>,
        stores: &GlobalStoreFactsV25<'_, '_>,
        launch: fe2o3_kernel_ir::ExplicitLaunchExtent,
        width: fe2o3_kernel_ir::FormalIndexWidth,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: &mut F,
    ) -> SourceOwnedResultV18<()>
    where
        F: for<'s, 'a, 'g> FnMut(
            &SourceSliceRootCompletionV25<'s, 'a, 'g>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<()>,
    {
        let original = self.original();
        original.global_expression_entry_v23(self.source.optimized(), budget)?;
        let run = |budget: &mut ArgumentBudgetV1<'work>| {
            let mut indexed = |index: &SourceSliceEntryIndexV26<'_, '_>,
                               budget: &mut ArgumentBudgetV1<'work>| {
                self.with_complete_indexed_slice_domains_v26::<PREDICATED, F>(
                    index, native, reads, stores, launch, width, budget, consume,
                )
            };
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of_val(&indexed),
                std::mem::align_of_val(&indexed),
            ])?)?;
            self.with_slice_entry_index_v26(budget, &mut indexed)
        };
        let frame = argument_sum_v1(&[std::mem::size_of_val(&run), std::mem::align_of_val(&run)])?;
        original.retain_query(source_scalar_normalization_scratch_v18(
            original.source.cleanup,
            budget,
            frame,
            run,
        ))
    }

    fn with_complete_indexed_slice_domains_v26<'work, const PREDICATED: bool, F>(
        &self,
        index: &SourceSliceEntryIndexV26<'_, '_>,
        native: &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>,
        reads: &GlobalReadFactsV18<'_, '_>,
        stores: &GlobalStoreFactsV25<'_, '_>,
        launch: fe2o3_kernel_ir::ExplicitLaunchExtent,
        width: fe2o3_kernel_ir::FormalIndexWidth,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: &mut F,
    ) -> SourceOwnedResultV18<()>
    where
        F: for<'s, 'a, 'g> FnMut(
            &SourceSliceRootCompletionV25<'s, 'a, 'g>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<()>,
    {
        let original = self.original();
        original.global_expression_entry_v23(self.source.optimized(), budget)?;
        budget.charge_work(1)?;
        if !std::ptr::eq(index.source, self) {
            return original
                .source
                .missing("slice completion argument index owner differs");
        }
        let output = self
            .source
            .optimized()
            .pending_global_output_v18(original, budget)?;
        original.retain_query(
            native
                .check_owner(output.owner(), budget)
                .map_err(slice_entry_native_error_v25),
        )?;
        let run = |budget: &mut ArgumentBudgetV1<'work>| -> SourceOwnedResultV18<()> {
            // Every callback and query carrier remains below this scratch floor.
            budget.reserve_storage(slice_completion_headers_v25::<F>()?)?;
            if PREDICATED {
                budget.reserve_storage(predicated_source_role_headers_v89::<F>()?)?;
            }
            let read_owner = reads
                .owner(budget)
                .map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?;
            let store_owner = stores
                .owner(budget)
                .map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?;
            if !std::ptr::eq(read_owner, output.owner())
                || !std::ptr::eq(store_owner, output.owner())
            {
                return original
                    .source
                    .missing("slice completion formal owner differs");
            }
            let count = self
                .profile
                .argument_count_v25(budget)
                .map_err(source_argument_error_v18)?;
            let mut arguments = source_reference_emission_vec_v29(count, budget)
                .map_err(source_argument_error_v18)?;
            budget.charge_work(count)?;
            arguments.resize(count, None);
            index.complete_parameters_v26(&mut arguments, budget)?;
            let mut observed = [0_usize; 2];
            for row in self.source.roles.rows {
                budget.charge_work(4)?;
                match row.role {
                    Some(DescriptorSourceRoleV18::Read) => {
                        self.source
                            .with_local_read_conditions_v18(
                                native,
                                reads,
                                row.output,
                                budget,
                                |read, budget| {
                                    let read =
                                        read.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                            "slice read occurrence is absent",
                                        ))?;
                                    let projection = stores
                                        .read_invocation_projection(&read.fact, budget)
                                        .map_err(|error| {
                                            self.source.local_read_formal_error(error)
                                        })?
                                        .map(|(axis, _)| axis);
                                    let access = PendingGlobalSourceNativeAccessV18 {
                                        pair: read.pair,
                                        native: read.native,
                                    };
                                    index.with_entry_v26(
                                        &access,
                                        budget,
                                        &mut |entry, budget| {
                                            slice_completion_add_v25(
                                                &mut arguments,
                                                entry,
                                                projection,
                                                budget,
                                            )
                                        },
                                    )?;
                                    observed[0] = observed[0]
                                        .checked_add(1)
                                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                                    Ok(())
                                },
                            )
                            .map_err(slice_completion_read_error_v25)?;
                    }
                    Some(DescriptorSourceRoleV18::Write) => {
                        self.source.with_native_access_profile_v89::<PREDICATED>(native, row.output, budget, |access, budget| {
                            let access = access.ok_or(ProductionSourceOwnedViewErrorV18::Binding("slice Store occurrence is absent"))?;
                            let outcome = stores.store_at(row.output, budget).map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?;
                            let fe2o3_kernel_ir::CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(fact) = outcome else {
                                return Err(ProductionSourceOwnedViewErrorV18::Binding("slice Store local conditions remain unproved").into());
                            };
                            if PREDICATED && matches!(access.pair.output.logical.guard, GlobalSourceGuardV85::ExplicitPredicate { .. }) {
                                self.source.check_predicated_store_domain_v89(stores, access.pair, &fact, budget)?;
                            } else {
                                self.source.check_local_store_domain_v25(stores, access.pair, &fact, budget)?;
                            }
                            let proof = fact.distinct_invocations(launch, width, budget)
                                .map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?
                                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("slice Store invocation injectivity remains unproved"))?;
                            index.with_entry_v26(access, budget, &mut |entry, budget| {
                                slice_completion_add_v25(&mut arguments, entry, Some(proof.projection().0), budget)
                            })?;
                            observed[1] = observed[1].checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                            Ok(())
                        }).map_err(slice_completion_native_error_v25)?;
                    }
                    _ => {}
                }
            }
            let function = optimized_source_root_function_v18(
                original,
                self.source.optimized(),
                self.root(),
                budget,
            )?;
            let read_effects = reads
                .function_effects(function.coordinate, budget)
                .map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?;
            let store_effects = stores
                .function_effects(function.coordinate, budget)
                .map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?;
            budget.charge_work(12)?;
            if read_effects != (observed[0], observed[1], 0)
                || store_effects != (observed[1], observed[0], 0)
            {
                return original
                    .source
                    .missing("slice global effect census is incomplete");
            }
            self.source.roles.check(budget)?;
            let completion = SourceSliceRootCompletionV25 {
                source: self,
                native,
                arguments: &arguments,
                launch,
                width,
            };
            shared_entry_consume_v18(original, budget, |budget| consume(&completion, budget))?;
            self.source.roles.check(budget)?;
            native
                .check_owner(output.owner(), budget)
                .map_err(slice_entry_native_error_v25)?;
            Ok(())
        };
        let frame = argument_sum_v1(&[std::mem::size_of_val(&run), std::mem::align_of_val(&run)])?;
        original.retain_query(source_scalar_normalization_scratch_v18(
            original.source.cleanup,
            budget,
            frame,
            run,
        ))
    }
}

fn slice_completion_headers_v25<F>() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        &'a PendingSharedEntryRegionsV18<'a, 'a>,
        &'a fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'a, 'a>,
        &'a GlobalReadFactsV18<'a, 'a>,
        &'a GlobalStoreFactsV25<'a, 'a>,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a mut ArgumentBudgetV1<'a>,
        Vec<Option<SourceSliceArgumentCompletionV25>>,
        &'a mut Vec<Option<SourceSliceArgumentCompletionV25>>,
        SourceSliceArgumentCompletionV25,
        Option<SourceSliceArgumentCompletionV25>,
        SourceSliceRootCompletionV25<'a, 'a, 'a>,
        PendingGlobalSourceNativeAccessV18<'a, 'a>,
        GlobalStoreFactV25<'a, 'a>,
        fe2o3_kernel_ir::CanonicalGuardedGlobalStoreOutcomeV24<'a, 'a>,
        fe2o3_kernel_ir::CanonicalGuardedStoreInjectivityV24<'a, 'a, 'a>,
        Option<fe2o3_kernel_ir::CanonicalGuardedStoreInjectivityV24<'a, 'a, 'a>>,
        Option<(fe2o3_kernel_ir::Axis, ValueId)>,
        Option<fe2o3_kernel_ir::Axis>,
        fe2o3_kernel_ir::ExplicitLaunchExtent,
        fe2o3_kernel_ir::FormalIndexWidth,
        [usize; 16],
        [&'a (); 24],
        PendingGlobalReadConditionErrorV18,
        PendingGlobalNativeErrorV18,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        argument_product_v1(2, size_of::<SourceOwnedResultV18<Frame<'_>>>())?,
        argument_product_v1(4, size_of::<&mut F>())?,
        slice_parameters_headers_v26()?,
    ])
}

fn slice_completion_ranked_error_v25(
    error: fe2o3_kernel_analysis::CanonicalRankedViewErrorV1,
) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        fe2o3_kernel_analysis::CanonicalRankedViewErrorV1::Resource(resource) => resource.into(),
        _ => ProductionSourceOwnedViewErrorV18::Binding("slice completion ranked view refused"),
    }
}

// Production family constructor: one imported native graph, one pair of local
// formal analyses and one original expression index serve every original root.
// The mixed-family finalizer must consume these rows alongside private memory;
// this constructor never promotes a report or publishes an output by itself.
fn with_source_slice_completion_v25<'work, F>(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    launches: &[fe2o3_kernel_ir::ExplicitLaunchExtent],
    width: fe2o3_kernel_ir::FormalIndexWidth,
    limits: fe2o3_kernel_ir::CanonicalGuardedGlobalReadLimitsV1,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: &mut F,
) -> SourceOwnedResultV18<()>
where
    F: for<'s, 'a, 'g> FnMut(
        &SourceSliceRootCompletionV25<'s, 'a, 'g>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
{
    use fe2o3_kernel_analysis::{
        CanonicalRankedMetadataV18, CanonicalRankedViewErrorV1,
        build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
    };
    original.global_expression_entry_v23(optimized, budget)?;
    let run = |budget: &mut ArgumentBudgetV1<'work>| -> SourceOwnedResultV18<()> {
        budget.reserve_storage(argument_sum_v1(&[
            slice_completion_headers_v25::<F>()?,
            size_of::<CanonicalRankedMetadataV18<'_, '_>>(),
            size_of::<Option<SourceOwnedResultV18<()>>>(),
            size_of::<Result<SourceOwnedResultV18<()>, CanonicalRankedViewErrorV1>>(),
            size_of::<fe2o3_kernel_ir::CanonicalGuardedGlobalReadLimitsV1>(),
            argument_product_v1(24, size_of::<&()>())?,
        ])?)?;
        optimized_source_endpoints_v18(original, optimized, budget)?;
        source_output_correspondence_checks_v18(original, optimized, budget)?;
        if launches.len() != original.source.root_count(budget)? {
            return original
                .source
                .missing("slice completion launch roster differs");
        }
        let output = optimized.output_inventory(budget)?;
        let metadata = CanonicalRankedMetadataV18::new(output.owner(), &[]);
        let metadata_storage = metadata
            .storage_extent(budget)
            .map_err(slice_completion_ranked_error_v25)?;
        budget.reserve_storage(metadata_storage)?;
        let (candidate, receipt) = build_canonical_ranked_candidate_v18(output, &metadata, budget)
            .map_err(slice_completion_ranked_error_v25)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let layouts = original.source.limits(budget)?.storage_layout_limits();
        with_checked_canonical_ranked_view_v18(output, &metadata, &candidate, budget, |checked, budget| {
            Ok::<_, CanonicalRankedViewErrorV1>(
                fe2o3_pliron::with_pending_canonical_ranked_source_roles_v18(checked, layouts, budget, |pending, budget| {
                    let mut result = None;
                    pending.with_pending_global_accesses_v18(output.owner(), budget, |native, budget| {
                        result = Some(fe2o3_kernel_ir::with_canonical_guarded_global_reads_v18(output.owner(), limits, budget, |reads, budget| {
                            Ok(fe2o3_kernel_ir::with_canonical_guarded_global_stores_v24(output.owner(), limits, budget, |stores, budget| {
                                Ok(original.with_global_source_expressions_v23(optimized, budget, &mut |root, source, budget| {
                                    source.with_shared_entry_regions_v18(budget, |entries, budget| {
                                        entries.with_complete_slice_domains_v25(native, reads, stores, launches[root], width, budget, consume)
                                    })
                                }))
                            }).map_err(|error| optimized_source_observed_formal_error_v18(original, &error)).and_then(|result| result))
                        }).map_err(|error| optimized_source_observed_formal_error_v18(original, &error)).and_then(|result| result));
                        Ok(())
                    })?;
                    Ok(result.unwrap_or_else(|| original.source.missing("slice completion native callback was not reached")))
                }).map_err(slice_entry_native_error_v25).and_then(|result| result)
            )
        }).map_err(slice_completion_ranked_error_v25)??;
        drop(candidate);
        drop(metadata);
        budget.release_storage(argument_sum_v1(&[
            metadata_storage,
            receipt.retained_storage(),
        ])?)?;
        optimized_source_endpoints_v18(original, optimized, budget)?;
        Ok(())
    };
    let frame = argument_sum_v1(&[std::mem::size_of_val(&run), std::mem::align_of_val(&run)])?;
    original.retain_query(source_scalar_normalization_scratch_v18(
        original.source.cleanup,
        budget,
        frame,
        run,
    ))
}
