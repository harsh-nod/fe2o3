// The source/native join does not replace the guarded Store-domain proof or a
// runtime allocation binding. Those obligations remain distinct from this ABI.
struct SourceSliceEntryRegionV25<'s, 'g> {
    root: usize,
    source: SourceSliceArgumentV26<'s>,
    abi: &'s kernel_argument_abi_v18::SourceSliceEntryAbiV25<'s>,
    access: &'s PendingGlobalSourceNativeAccessV18<'s, 'g>,
}

impl SourceSliceEntryRegionV25<'_, '_> {
    fn original_argument(&self) -> u32 {
        self.source.source_argument()
    }
    fn requires_initialized_extent(&self) -> bool {
        !self.access.pair.output.writing
    }
    fn requires_exclusive_runtime_binding(&self) -> bool {
        self.abi.is_exclusive_contract()
    }

    fn original_parameter(&self) -> SliceDefinition {
        self.access.pair.input.logical.root
    }

    fn optimized_parameter(&self) -> SliceDefinition {
        self.access.pair.output.logical.root
    }

    fn original_access(&self) -> SliceOperation {
        self.access.pair.input.logical.access.operation
    }

    fn optimized_access(&self) -> SliceOperation {
        self.access.pair.output.logical.access.operation
    }
    const fn grants_memory_or_launch_authority(&self) -> bool {
        false
    }
}

#[cfg(test)]
include!("production_optimized_source_slice_entry_commands_v25_tests.rs");

include!("production_optimized_source_slice_domains_v25.rs");
include!("production_optimized_source_slice_entry_index_v26.rs");

fn slice_entry_region_headers_v25() -> Result<usize, ArgumentResourceV1> {
    use kernel_argument_abi_v18::{SourceDescriptorRootAbiV29, SourceSliceEntryAbiV25};
    type Frame<'a> = (
        &'a PendingSharedEntryRegionsV18<'a, 'a>,
        &'a PendingGlobalSourceNativeAccessV18<'a, 'a>,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a GlobalSourceAccessPairV18,
        &'a mut ArgumentBudgetV1<'a>,
        SourceDescriptorRootAbiV29<'a>,
        SourceSliceEntryAbiV25<'a>,
        Option<SourceSliceEntryAbiV25<'a>>,
        &'a SourceSliceEntryAbiV25<'a>,
        SourceSliceEntryRegionV25<'a, 'a>,
        &'a SourceSliceEntryRegionV25<'a, 'a>,
        &'a CanonicalKirDefinitionRefV1<'a>,
        &'a CanonicalKirDefinitionRefV1<'a>,
        SourceOwnedResultV18<&'a CanonicalKirDefinitionRefV1<'a>>,
        ProductionArgumentNodeV1<'a>,
        SourceSliceArgumentV26<'a>,
        &'a ProductionArgumentNodeV1<'a>,
        Option<(u32, SemanticTypeIdV1)>,
        (u32, SemanticTypeIdV1),
        [usize; 5],
        ValueId,
        Option<ValueId>,
        &'a Type,
        SliceDefinition,
        SemanticFunctionIdV1,
        bool,
        Result<usize, std::num::TryFromIntError>,
        Result<(), ProductionSemanticKirErrorV1>,
        Result<(), fe2o3_pliron::CanonicalRankedPolicyFailureV1>,
        fe2o3_pliron::CanonicalRankedPolicyFailureV1,
        Option<ArgumentResourceV1>,
    );
    argument_sum_v1(&[
        kernel_argument_abi_v18::slice_entry_abi_headers_v25()?,
        size_of::<Frame<'_>>(),
        argument_product_v1(
            2,
            size_of::<Result<Frame<'_>, ProductionSourceOwnedViewErrorV18>>(),
        )?,
    ])
}

fn slice_entry_native_error_v25(
    error: fe2o3_pliron::CanonicalRankedPolicyFailureV1,
) -> ProductionSourceOwnedViewErrorV18 {
    match ProductionOptimizedExecutionRecipesV18::policy_resource(&error) {
        Some(resource) => resource.into(),
        None => {
            ProductionSourceOwnedViewErrorV18::Binding("slice entry native owner or epoch differs")
        }
    }
}

impl PendingSharedEntryRegionsV18<'_, '_> {
    // The containing with_native_access_v18 scope authenticates all four actual
    // native operations and keeps its graph epoch live through this callback.
    fn with_slice_entry_region_v25<'work, F>(
        &self,
        access: &PendingGlobalSourceNativeAccessV18<'_, '_>,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: &mut F,
    ) -> SourceOwnedResultV18<()>
    where
        F: for<'s, 'g> FnMut(
            &SourceSliceEntryRegionV25<'s, 'g>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<()>,
    {
        let original = self.original();
        original.global_expression_entry_v23(self.source.optimized(), budget)?;
        original.retain_query(self.source.roles.observe_custody(budget))?;
        let output = self
            .source
            .optimized()
            .pending_global_output_v18(original, budget)?;
        original.retain_query(
            access
                .native
                .check_owner(output.owner(), budget)
                .map_err(slice_entry_native_error_v25),
        )?;
        original.retain_query(
            self.arguments
                .check(budget)
                .map_err(source_argument_error_v18),
        )?;
        let run = |budget: &mut ArgumentBudgetV1<'work>| -> SourceOwnedResultV18<()> {
            budget.reserve_storage(slice_entry_region_headers_v25()?)?;
            let pair = access.pair;
            budget.charge_work(18)?;
            // Authenticate this pair through this exact source role scope, not
            // only its numerically equal parameter coordinates.
            if self
                .source
                .access(pair.output.logical.access.operation, budget)?
                != Some(pair)
            {
                return original.source.missing("slice entry source access differs");
            }
            let SliceDefinition::FunctionArgument { function, argument } = pair.input.logical.root
            else {
                return original
                    .source
                    .missing("slice entry has no original root parameter");
            };
            if function.0 as usize != self.physical_function {
                return original
                    .source
                    .missing("slice entry original function differs");
            }
            let input = optimized_source_definition_row_v18(
                original.inventory,
                pair.input.logical.root,
                budget,
            )?;
            let output_inventory = self.source.optimized().output_inventory(budget)?;
            let output = optimized_source_definition_row_v18(
                output_inventory,
                pair.output.logical.root,
                budget,
            )?;
            let value = input
                .value
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "slice entry original parameter has no value",
                ))?;
            if input.ty != output.ty || pair.input.writing != pair.output.writing {
                return original
                    .source
                    .missing("slice entry output type or access differs");
            }
            let slot = usize::try_from(argument).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            let mut selected = None;
            let mut count = 0usize;
            let mut select = |node: ProductionArgumentNodeV1<'_>,
                              budget: &mut ArgumentBudgetV1<'_>| {
                budget.charge_work(8)?;
                if shared_entry_node_matches_v18(&node, slot, value, input.ty) {
                    count = count.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                    selected = Some((node.source_argument(), node.semantic_type()));
                }
                Ok(())
            };
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of_val(&select),
                std::mem::align_of_val(&select),
            ])?)?;
            self.arguments
                .visit_nodes_scoped(budget, &mut select)
                .map_err(source_argument_error_v18)?;
            drop(select);
            if count != 1 {
                return original
                    .source
                    .missing("slice entry has no unique original argument");
            }
            let (ordinal, ty) = selected.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "slice entry argument is absent",
            ))?;
            let abi = self
                .profile
                .slice_entry_v25(ordinal, ty, input.ty, pair.input.writing, budget)
                .map_err(source_argument_error_v18)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "slice entry requires an authenticated slice descriptor",
                ))?;
            budget.charge_work(8)?;
            if abi.function() != self.original_function
                || abi.argument() != ordinal
                || abi.ty() != ty
                || abi.scalar() != pair.input.scalar
                || abi.scalar() != pair.output.scalar
            {
                return original
                    .source
                    .missing("slice entry descriptor differs from source access");
            }
            let mut invoked = false;
            let mut visit = |node: ProductionArgumentNodeV1<'_>,
                             budget: &mut ArgumentBudgetV1<'work>| {
                budget.charge_work(8)?;
                if shared_entry_node_matches_v18(&node, slot, value, input.ty) {
                    if invoked || node.source_argument() != ordinal || node.semantic_type() != ty {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    invoked = true;
                    let region = SourceSliceEntryRegionV25 {
                        root: self.root(),
                        source: SourceSliceArgumentV26::Scoped(node),
                        abi: &abi,
                        access,
                    };
                    let call = |budget: &mut ArgumentBudgetV1<'work>| consume(&region, budget);
                    budget.reserve_storage(argument_sum_v1(&[
                        std::mem::size_of_val(&call),
                        std::mem::align_of_val(&call),
                    ])?)?;
                    shared_entry_consume_v18(original, budget, call)
                        .map_err(source_slice_query_error_v18)?;
                }
                Ok(())
            };
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of_val(&visit),
                std::mem::align_of_val(&visit),
            ])?)?;
            self.arguments
                .visit_nodes_scoped(budget, &mut visit)
                .map_err(source_argument_error_v18)?;
            drop(visit);
            if !invoked {
                return original
                    .source
                    .missing("slice entry argument was not consumed");
            }
            Ok(())
        };
        let frame = argument_sum_v1(&[std::mem::size_of_val(&run), std::mem::align_of_val(&run)])?;
        let result =
            source_scalar_normalization_scratch_v18(original.source.cleanup, budget, frame, run);
        let result = original.retain_query(result);
        let postflight = original.retain_query(
            access
                .native
                .check_owner(output.owner(), budget)
                .map_err(slice_entry_native_error_v25),
        );
        result.and(postflight)
    }
}
