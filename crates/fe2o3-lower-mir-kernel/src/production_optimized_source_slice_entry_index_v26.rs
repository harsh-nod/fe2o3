// The index copies only inert node identities during one authentic scoped walk.
// Its private constructor and retained argument view provide custody; copied
// ordinals by themselves never manufacture a source argument or runtime grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceSliceIndexedArgumentV26 {
    argument: u32,
    ty: SemanticTypeIdV1,
    slot: usize,
    value: ValueId,
}

enum SourceSliceArgumentV26<'s> {
    Scoped(ProductionArgumentNodeV1<'s>),
    Indexed {
        arguments: &'s ArgumentViewDataV18<'s>,
        node: &'s SourceSliceIndexedArgumentV26,
    },
}

impl SourceSliceArgumentV26<'_> {
    fn source_argument(&self) -> u32 {
        match self {
            Self::Scoped(node) => node.source_argument(),
            Self::Indexed { node, .. } => node.argument,
        }
    }

    fn semantic_type(&self) -> SemanticTypeIdV1 {
        match self {
            Self::Scoped(node) => node.semantic_type(),
            Self::Indexed { node, .. } => node.ty,
        }
    }
}

struct SourceSliceEntryIndexV26<'s, 'a> {
    source: &'s PendingSharedEntryRegionsV18<'s, 'a>,
    rows: &'s [Option<SourceSliceIndexedArgumentV26>],
}

impl PendingSharedEntryRegionsV18<'_, '_> {
    fn with_slice_entry_index_v26<'work, F>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: &mut F,
    ) -> SourceOwnedResultV18<()>
    where
        F: for<'s, 'a> FnMut(
            &SourceSliceEntryIndexV26<'s, 'a>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<()>,
    {
        let original = self.original();
        original.global_expression_entry_v23(self.source.optimized(), budget)?;
        original.retain_query(self.source.roles.observe_custody(budget))?;
        original.retain_query(
            self.arguments
                .check(budget)
                .map_err(source_argument_error_v18),
        )?;
        let run = |budget: &mut ArgumentBudgetV1<'work>| -> SourceOwnedResultV18<()> {
            budget.reserve_storage(slice_entry_index_headers_v26::<F>()?)?;
            budget.charge_work(8)?;
            let function = original
                .inventory
                .functions()
                .get(self.physical_function)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "slice index original function is absent",
                ))?;
            let count = function.function.signature.parameters.len();
            let mut rows = source_reference_emission_vec_v29(count, budget)
                .map_err(source_argument_error_v18)?;
            budget.charge_work(count)?;
            rows.resize(count, None);
            let mut visit = |node: ProductionArgumentNodeV1<'_>,
                             budget: &mut ArgumentBudgetV1<'work>| {
                budget.charge_work(12)?;
                let ProductionArgumentCoverageV1::Parameter(parameter) = node.coverage() else {
                    return Ok(());
                };
                if !matches!(parameter.ty(), Type::Slice(_))
                    || !node.source_path().is_empty()
                    || !node
                        .local_binding()
                        .is_some_and(|(_, path)| path.is_empty())
                {
                    return Ok(());
                }
                let row = rows
                    .get_mut(parameter.slot())
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                if row.is_some() {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let physical = self
                    .arguments
                    .physical(parameter.slot(), budget)?
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                let same_trace = match (physical.trace(), parameter.trace()) {
                    (
                        source_arguments_v1::ProductionArgumentTraceV1::Direct(a),
                        source_arguments_v1::ProductionArgumentTraceV1::Direct(b),
                    ) => std::ptr::eq(a, b),
                    (
                        source_arguments_v1::ProductionArgumentTraceV1::Component(a),
                        source_arguments_v1::ProductionArgumentTraceV1::Component(b),
                    ) => std::ptr::eq(a, b),
                    _ => false,
                };
                if physical.slot() != parameter.slot()
                    || physical.value() != parameter.value()
                    || !std::ptr::eq(physical.ty(), parameter.ty())
                    || !same_trace
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                *row = Some(SourceSliceIndexedArgumentV26 {
                    argument: node.source_argument(),
                    ty: node.semantic_type(),
                    slot: parameter.slot(),
                    value: parameter.value(),
                });
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
            let index = SourceSliceEntryIndexV26 {
                source: self,
                rows: &rows,
            };
            let call = |budget: &mut ArgumentBudgetV1<'work>| consume(&index, budget);
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of_val(&call),
                std::mem::align_of_val(&call),
            ])?)?;
            shared_entry_consume_v18(original, budget, call)?;
            self.arguments
                .check(budget)
                .map_err(source_argument_error_v18)?;
            self.source.roles.check(budget)?;
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

impl SourceSliceEntryIndexV26<'_, '_> {
    fn with_entry_v26<'work, F>(
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
        let source = self.source;
        let original = source.original();
        original.global_expression_entry_v23(source.source.optimized(), budget)?;
        original.retain_query(source.source.roles.observe_custody(budget))?;
        original.retain_query(
            source
                .arguments
                .check(budget)
                .map_err(source_argument_error_v18),
        )?;
        let output = source
            .source
            .optimized()
            .pending_global_output_v18(original, budget)?;
        original.retain_query(
            access
                .native
                .check_owner(output.owner(), budget)
                .map_err(slice_entry_native_error_v25),
        )?;
        let run = |budget: &mut ArgumentBudgetV1<'work>| -> SourceOwnedResultV18<()> {
            budget.reserve_storage(argument_sum_v1(&[
                slice_entry_region_headers_v25()?,
                slice_entry_index_headers_v26::<F>()?,
            ])?)?;
            budget.charge_work(24)?;
            let pair = access.pair;
            if source
                .source
                .access(pair.output.logical.access.operation, budget)?
                != Some(pair)
            {
                return original
                    .source
                    .missing("indexed slice source access differs");
            }
            let SliceDefinition::FunctionArgument { function, argument } = pair.input.logical.root
            else {
                return original
                    .source
                    .missing("indexed slice has no original root parameter");
            };
            if function.0 as usize != source.physical_function {
                return original
                    .source
                    .missing("indexed slice original function differs");
            }
            let input = optimized_source_definition_row_v18(
                original.inventory,
                pair.input.logical.root,
                budget,
            )?;
            let result =
                optimized_source_definition_row_v18(output, pair.output.logical.root, budget)?;
            if input.ty != result.ty || pair.input.writing != pair.output.writing {
                return original
                    .source
                    .missing("indexed slice output type or access differs");
            }
            let slot = usize::try_from(argument).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            let row = self.rows.get(slot).and_then(Option::as_ref).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("indexed slice argument is absent"),
            )?;
            if row.slot != slot || input.value != Some(row.value) {
                return original
                    .source
                    .missing("indexed slice original parameter differs");
            }
            let abi = source
                .profile
                .slice_entry_v25(row.argument, row.ty, input.ty, pair.input.writing, budget)
                .map_err(source_argument_error_v18)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "indexed slice requires an authenticated descriptor",
                ))?;
            if abi.function() != source.original_function
                || abi.argument() != row.argument
                || abi.ty() != row.ty
                || abi.scalar() != pair.input.scalar
                || abi.scalar() != pair.output.scalar
            {
                return original
                    .source
                    .missing("indexed slice descriptor differs from source access");
            }
            let region = SourceSliceEntryRegionV25 {
                root: source.root(),
                source: SourceSliceArgumentV26::Indexed {
                    arguments: &source.arguments,
                    node: row,
                },
                abi: &abi,
                access,
            };
            let call = |budget: &mut ArgumentBudgetV1<'work>| consume(&region, budget);
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of_val(&call),
                std::mem::align_of_val(&call),
            ])?)?;
            shared_entry_consume_v18(original, budget, call)?;
            source
                .arguments
                .check(budget)
                .map_err(source_argument_error_v18)?;
            source.source.roles.check(budget)?;
            Ok(())
        };
        let frame = argument_sum_v1(&[std::mem::size_of_val(&run), std::mem::align_of_val(&run)])?;
        let result = original.retain_query(source_scalar_normalization_scratch_v18(
            original.source.cleanup,
            budget,
            frame,
            run,
        ));
        let postflight = original.retain_query(
            access
                .native
                .check_owner(output.owner(), budget)
                .map_err(slice_entry_native_error_v25),
        );
        result.and(postflight)
    }
}

fn slice_entry_index_headers_v26<F>() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        &'a PendingSharedEntryRegionsV18<'a, 'a>,
        &'a SourceSliceEntryIndexV26<'a, 'a>,
        SourceSliceEntryIndexV26<'a, 'a>,
        Vec<Option<SourceSliceIndexedArgumentV26>>,
        &'a mut Vec<Option<SourceSliceIndexedArgumentV26>>,
        Option<SourceSliceIndexedArgumentV26>,
        &'a mut Option<SourceSliceIndexedArgumentV26>,
        Option<&'a SourceSliceIndexedArgumentV26>,
        &'a SourceSliceIndexedArgumentV26,
        SourceSliceIndexedArgumentV26,
        ProductionArgumentNodeV1<'a>,
        ProductionArgumentCoverageV1<'a>,
        ProductionPhysicalArgumentV1<'a>,
        source_arguments_v1::ProductionArgumentTraceV1<'a>,
        source_arguments_v1::ProductionArgumentTraceV1<'a>,
        bool,
        Option<ProductionPhysicalArgumentV1<'a>>,
        Result<Option<ProductionPhysicalArgumentV1<'a>>, ProductionSemanticKirErrorV1>,
        &'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'a>,
        Option<&'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'a>>,
        SourceSliceArgumentV26<'a>,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a mut ArgumentBudgetV1<'a>,
        [usize; 4],
        SourceOwnedResultV18<()>,
        Result<(), ProductionSemanticKirErrorV1>,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        argument_product_v1(2, size_of::<SourceOwnedResultV18<Frame<'_>>>())?,
        argument_product_v1(2, size_of::<&mut F>())?,
    ])
}
