// Seed declarations before visiting effects. A zero count is proved only after
// the complete source effect walk and the native conditional census agree.
impl SourceSliceEntryIndexV26<'_, '_> {
    fn complete_parameters_v26(
        &self,
        rows: &mut [Option<SourceSliceArgumentCompletionV25>],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let source = self.source;
        let original = source.original();
        let optimized = source.source.optimized();
        optimized_source_endpoints_v18(original, optimized, budget)?;
        source
            .arguments
            .check(budget)
            .map_err(source_argument_error_v18)?;
        budget.charge_work(8)?;
        let input_function = original
            .inventory
            .functions()
            .get(source.physical_function)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "slice parameter original root is absent",
            ))?;
        let output_function =
            optimized_source_root_function_v18(original, optimized, source.root(), budget)?;
        let output = optimized.output_inventory(budget)?;
        if self.rows.len() != input_function.function.signature.parameters.len()
            || rows.len()
                != source
                    .profile
                    .argument_count_v25(budget)
                    .map_err(source_argument_error_v18)?
        {
            return original
                .source
                .missing("slice parameter census extent differs");
        }
        for (slot, indexed) in self.rows.iter().enumerate() {
            budget.charge_work(12)?;
            let Some(indexed) = indexed else { continue };
            let input_coordinate = SliceDefinition::FunctionArgument {
                function: input_function.coordinate,
                argument: u32::try_from(slot).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            };
            let input =
                optimized_source_definition_row_v18(original.inventory, input_coordinate, budget)?;
            if indexed.slot != slot || input.value != Some(indexed.value) {
                return original
                    .source
                    .missing("slice parameter original definition differs");
            }
            let Some(abi) = source
                .profile
                .slice_parameter_v26(indexed.argument, indexed.ty, input.ty, budget)
                .map_err(source_argument_error_v18)?
            else {
                continue;
            };
            if abi.function() != source.original_function
                || abi.argument() != indexed.argument
                || abi.ty() != indexed.ty
            {
                return original
                    .source
                    .missing("slice parameter source ABI differs");
            }
            // A declaration bijection identifies the output root. The checked
            // descendant relation identifies its actual parameter, not a
            // coincidentally equal original physical or logical ordinal.
            let mut parameter = None;
            for descendant in optimized.definition_descendants(input_coordinate, budget)? {
                budget.charge_work(4)?;
                if let SliceDefinition::FunctionArgument { function, .. } = descendant.output
                    && function == output_function.coordinate
                {
                    if parameter.replace(descendant.output).is_some() {
                        return original
                            .source
                            .missing("slice parameter has repeated output declarations");
                    }
                }
            }
            let parameter = parameter.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "slice parameter has no output declaration",
            ))?;
            let definition = optimized_source_definition_row_v18(output, parameter, budget)?;
            if definition.ty != input.ty {
                return original
                    .source
                    .missing("slice parameter output type differs");
            }
            let ordinal =
                usize::try_from(indexed.argument).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            let row = rows
                .get_mut(ordinal)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "slice parameter original argument is absent",
                ))?;
            if row.is_some() {
                return original
                    .source
                    .missing("slice parameter original argument is repeated");
            }
            *row = Some(SourceSliceArgumentCompletionV25 {
                parameter,
                ty: abi.ty(),
                identity: abi.identity(),
                scalar: abi.scalar(),
                exclusive: abi.is_exclusive_contract(),
                reads: 0,
                writes: 0,
                axis: None,
                different_projection: false,
            });
        }
        // Replay the complete captured source roster, including descriptors
        // which never occur in any executable address or memory operation.
        for (ordinal, row) in rows.iter().enumerate() {
            budget.charge_work(4)?;
            let argument = u32::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            let expected = source
                .profile
                .slice_parameter_type_v26(argument, budget)
                .map_err(source_argument_error_v18)?;
            if expected != row.as_ref().map(|row| row.ty) {
                return original
                    .source
                    .missing("slice parameter source roster is incomplete");
            }
        }
        source
            .arguments
            .check(budget)
            .map_err(source_argument_error_v18)?;
        source.source.roles.check(budget)?;
        Ok(())
    }
}

fn slice_parameters_headers_v26() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        &'a SourceSliceEntryIndexV26<'a, 'a>,
        &'a PendingSharedEntryRegionsV18<'a, 'a>,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        [&'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'a>; 2],
        [&'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>; 2],
        &'a mut [Option<SourceSliceArgumentCompletionV25>],
        &'a SourceSliceIndexedArgumentV26,
        &'a mut Option<SourceSliceArgumentCompletionV25>,
        SourceSliceArgumentCompletionV25,
        [SliceDefinition; 2],
        Option<SliceDefinition>,
        Option<SemanticTypeIdV1>,
        [usize; 4],
        [u32; 2],
        std::iter::Enumerate<std::slice::Iter<'a, Option<SourceSliceIndexedArgumentV26>>>,
        std::iter::Enumerate<std::slice::Iter<'a, Option<SourceSliceArgumentCompletionV25>>>,
        std::slice::Iter<'a, fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1>,
        &'a mut ArgumentBudgetV1<'a>,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        argument_product_v1(2, size_of::<SourceOwnedResultV18<Frame<'_>>>())?,
        kernel_argument_abi_v18::slice_entry_abi_headers_v25()?,
    ])
}
