// Call-result transport is separate from interpreting the callee computation.
// The source SSA archive binds the exact original result; no new symbol is made.
fn source_call_return_headers_v32() -> Result<usize, ArgumentResourceV1> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
    type Frame<'a> = (
        [&'a (); 32],
        [usize; 24],
        [u32; 6],
        [bool; 3],
        OriginalEntryDefinitionRowV20,
        EntryValueV20,
        SourceScalarBoundaryV31,
        SemanticFunctionIdV1,
        SemanticTypeIdV1,
        Type,
        fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
        [Definition; 2],
        [Option<Definition>; 2],
        [Result<Option<Definition>, fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1>; 2],
        SourceOwnedResultV18<&'a SemanticDirectCallV1>,
        SourceOwnedResultV18<(SemanticFunctionIdV1, Option<(usize, SemanticBlockIdV1)>)>,
        SourceOwnedResultV18<(SemanticFunctionIdV1, usize)>,
        SourceOwnedResultV18<Option<usize>>,
        SourceOwnedResultV18<OriginalEntryDefinitionRowV20>,
        SourceOwnedResultV18<bool>,
        SourceOwnedResultV18<()>,
        Result<Type, ProductionSemanticKirErrorV1>,
        Result<
            Option<&'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>>,
            fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
        >,
        SourceOwnedResultV18<&'a [fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1]>,
        std::slice::Iter<'a, fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1>,
    );
    argument_sum_v1(&[size_of::<Frame<'_>>(), std::mem::align_of::<Frame<'_>>()])
}

fn source_entry_call_return_v32<'a>(
    source: &'a ProductionSourceCorrespondenceV18<'_>,
    function: SemanticFunctionIdV1,
    edge: usize,
    local: u32,
    definition: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<&'a SemanticDirectCallV1> {
    source.retain_query((|| {
        source.query(budget)?;
        let owner = source.source.source_ssa(budget)?;
        let rows = owner.occurrences_v1().and_then(|rows| rows.function(function)).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("source call-return occurrence owner"),
        )?;
        let occurrence = rows.edge_definitions().get(edge).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("source call-return edge occurrence"),
        )?;
        budget.charge_work(14)?;
        if !occurrence.is_reachable()
            || !occurrence.is_promoted()
            || occurrence.edge().ordinal() != 0
            || occurrence.ordinal() != 0
            || occurrence.variable().get() != local
            || !matches!(occurrence.value(), Some(EntryValueV20::Definition(value)) if value.get() == definition)
        {
            return source.source.missing("source call-return definition or edge differs");
        }
        let declaration = owner.source_semantic().functions().get(function.index() as usize).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("source call-return declaration"),
        )?;
        let Some(SemanticTerminatorKindV1::Call(call)) = declaration.blocks()
            .get(occurrence.edge().source().get() as usize).map(|block| block.terminator().kind())
        else {
            return source.source.missing("source edge definition is not a call return");
        };
        let destination = call.destination().ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("source call-return destination absent"),
        )?;
        // The retained capture emits successors in (source block, ordinal)
        // order. Resolve the exact occurrence, including its role and target.
        let (mut lo, mut hi) = (0, rows.successors().len());
        while lo < hi {
            budget.charge_work(1)?;
            let middle = lo + (hi - lo) / 2;
            if rows.successors()[middle].id() < occurrence.edge() {
                lo = middle + 1;
            } else {
                hi = middle;
            }
        }
        let successor = rows.successors().get(lo).filter(|row| row.id() == occurrence.edge()).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("source call-return successor absent"),
        )?;
        budget.charge_work(3)?;
        if destination.edge().role() != SemanticEdgeRoleV1::CallReturn
            || successor.edge() != destination.edge()
            || destination.edge().target().index() as usize >= declaration.blocks().len()
            || destination.place().local().index() != local
            || !destination.place().projections().is_empty()
            || declaration.locals().get(local as usize).map(|row| row.ty()) != Some(destination.place().ty())
        {
            return source.source.missing("source call-return destination or type differs");
        }
        Ok(call)
    })())
}

impl SourceBoundaryCheckV31<'_> {
    fn call_return_value_v32(
        &self,
        row: &SourceScalarBoundaryV31,
        original: EntryValueV20,
        actual: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<bool> {
        let EntryValueV20::Definition(value) = original else {
            return Ok(false);
        };
        let relation = self.leaves.relation;
        let (function, _) = relation
            .source
            .instance(self.leaves.root, row.instance, budget)?;
        let definition = self.index.definition(function, original, budget)?;
        let OriginalEntryDefinitionV20::CallReturn { block, edge } = definition.origin else {
            return Ok(false);
        };
        let call = source_entry_call_return_v32(
            relation,
            function,
            edge,
            definition.local,
            value.get(),
            budget,
        )?;
        budget.charge_work(9)?;
        if definition.key != [function.index(), value.get()]
            || definition.local != row.variable.get()
            || !matches!(relation.source.source_ssa(budget)?.occurrences_v1()
                .and_then(|rows| rows.function(function))
                .and_then(|rows| rows.edge_definitions().get(edge).map(|row| row.edge().source().get())),
                Some(actual_block) if actual_block == block)
            || call
                .destination()
                .map(|destination| destination.place().ty())
                != Some(row.ty)
        {
            return relation
                .source
                .missing("source SSA call-return boundary identity differs");
        }
        let input_index = relation
            .ssa_scalar_definition_v30(self.leaves.root, row.instance, original, budget)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source SSA call-return scalar locator absent",
            ))?;
        let input = relation.inventory.definitions().get(input_index).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding(
                "source SSA call-return scalar definition absent",
            ),
        )?;
        let input_function = relation
            .inventory
            .functions()
            .get(relation.source.root(self.leaves.root, budget)?.1)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source SSA call-return source function absent",
            ))?;
        let input_value = input
            .value
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source SSA call-return source value absent",
            ))?;
        let output_function = self.optimized.map_or(input_function.coordinate, |leaves| {
            leaves.function.coordinate
        });
        let expected_type =
            lower_scalar_type(relation.source.source_semantic(budget)?.types(), row.ty)
                .map_err(source_emission_error_v18)?;
        if !input_function.definitions.contains(&input_index)
            || input.ty != &expected_type
            || kir_semantic_scalar_v1(input.ty) != Some(row.scalar)
            || !self.origins.belongs_to(self.inventory, output_function)
        {
            return relation
                .source
                .missing("source SSA call-return scalar type or owner differs");
        }
        let actual_definition = self
            .inventory
            .definition_for_value(output_function, actual, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source SSA call-return target definition absent",
            ))?;
        if actual_definition.ty != input.ty {
            return relation
                .source
                .missing("source SSA call-return target type or owner differs");
        }
        let actual_origin = self
            .origins
            .resolve(actual, budget)
            .map_err(source_pointer_inventory_error_v18)?;
        let matched = match self.optimized {
            None => {
                let expected = self
                    .origins
                    .resolve(input_value, budget)
                    .map_err(source_pointer_inventory_error_v18)?;
                actual_definition.coordinate == input.coordinate
                    || actual_origin.is_some() && actual_origin == expected
            }
            Some(leaves) => {
                let mut matched = false;
                for descendant in leaves
                    .optimized
                    .definition_descendants(input.coordinate, budget)?
                {
                    budget.charge_work(5)?;
                    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
                    let function = match descendant.output {
                        Definition::FunctionArgument { function, .. } => function,
                        Definition::BlockArgument { block, .. } => block.function,
                        Definition::Result { operation, .. } => operation.block.function,
                    };
                    let output = optimized_source_definition_row_v18(
                        self.inventory,
                        descendant.output,
                        budget,
                    )?;
                    if function != output_function || output.ty != input.ty {
                        return relation
                            .source
                            .missing("source SSA call-return descendant type or owner differs");
                    }
                    let value = output
                        .value
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "source SSA call-return descendant value absent",
                        ))?;
                    let expected = self
                        .origins
                        .resolve(value, budget)
                        .map_err(source_pointer_inventory_error_v18)?;
                    matched |= actual_definition.coordinate == descendant.output
                        || actual_origin.is_some() && actual_origin == expected;
                }
                matched
            }
        };
        if !matched {
            return relation
                .source
                .missing("source SSA call-return incoming value differs");
        }
        Ok(true)
    }
}
