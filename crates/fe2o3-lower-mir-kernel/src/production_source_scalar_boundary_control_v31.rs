struct SourceBoundaryCheckV31<'a> {
    leaves: &'a SourceScalarLeavesV18<'a, 'a>,
    index: &'a OriginalEntryIndexV20<'a, 'a>,
    arguments: &'a SourceRootArgumentsV18<'a, 'a>,
    origins: &'a value_origin_v1::WholeValueOriginsV18<'a>,
    inline: &'a Gfx942InlineScalarCorrespondenceV30<'a>,
    optimized: Option<&'a ProductionOptimizedSourceScalarLeavesV18<'a>>,
    inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
    bindings: std::ops::Range<usize>,
}

fn source_boundary_function_seen_v31(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(std::ops::Range<usize>, Vec<bool>)> {
    budget.charge_work(3)?;
    let row = inventory.functions().get(function.0 as usize).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding(
            "source SSA boundary function binding range is absent",
        ),
    )?;
    if row.coordinate != function
        || row.edge_arguments.start > row.edge_arguments.end
        || row.edge_arguments.end > inventory.edge_arguments().len()
    {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "source SSA boundary function binding range differs",
        ));
    }
    let bindings = row.edge_arguments.clone();
    let mut seen = emission_vec_v1(bindings.len(), budget).map_err(source_emission_error_v18)?;
    budget.charge_work(bindings.len())?;
    seen.resize(bindings.len(), false);
    Ok((bindings, seen))
}

fn source_boundary_seen_index_v31(
    bindings: &std::ops::Range<usize>,
    binding: usize,
) -> SourceOwnedResultV18<usize> {
    binding
        .checked_sub(bindings.start)
        .filter(|index| *index < bindings.len())
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source SSA physical argument belongs to another function",
        ))
}

fn source_boundary_control_headers_v31() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a, 'w> = (
        SourceBoundaryCheckV31<'a>,
        OriginalEntryIndexV20<'a, 'a>,
        SourceRootArgumentsV18<'a, 'a>,
        Gfx942InlineScalarCorrespondenceV30<'a>,
        Option<Gfx942InlineScalarCorrespondenceV30<'a>>,
        SourceOwnedResultV18<Option<Gfx942InlineScalarCorrespondenceV30<'a>>>,
        OptimizedSourceScalarNormalizationV18<'a>,
        Vec<SourceReferenceSelectionControlV30>,
        Vec<bool>,
        std::ops::Range<usize>,
        (std::ops::Range<usize>, Vec<bool>),
        SourceOwnedResultV18<(std::ops::Range<usize>, Vec<bool>)>,
        SourceOwnedResultV18<usize>,
        [SourceOwnedResultV18<Option<&'a SourceScalarBoundaryV31>>; 3],
        [Option<&'a SourceScalarBoundaryV31>; 2],
        [&'a SourceScalarBoundaryV31; 2],
        [&'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>; 2],
        &'a fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1,
        &'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'a>,
        SourceOwnedResultV18<OriginalEntryIndexV20<'a, 'a>>,
        SourceOwnedResultV18<SourceRootArgumentsV18<'a, 'a>>,
        SourceOwnedResultV18<Gfx942InlineScalarCorrespondenceV30<'a>>,
        SourceOwnedResultV18<Vec<SourceReferenceSelectionControlV30>>,
        SourceOwnedResultV18<Vec<bool>>,
        Result<Vec<SourceReferenceSelectionControlV30>, ProductionSemanticKirErrorV1>,
        Result<Vec<bool>, ProductionSemanticKirErrorV1>,
        OriginalPrivateInputV22<'a>,
        OriginalEntryDefinitionRowV20,
        ProductionSemanticExpressionV2,
        SourceOwnedResultV18<ProductionSemanticExpressionV2>,
        SourceScalarBoundaryV31,
        InvocationArgumentRowV1,
        InvocationComponentRowV1,
        ScopedEmittedPointsV29<'a, 'a, 'w>,
        &'a fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirEdgeRefV1<'a>,
        &'a SemanticTerminatorKindV1,
        &'a Terminator,
        &'a mut ArgumentBudgetV1<'w>,
        [usize; 16],
        [SourceOwnedResultV18<()>; 4],
        Result<Option<(BlockId, u32)>, ScopedTileFailureKindV29>,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_, '_>>(),
        std::mem::align_of::<Frame<'_, '_>>(),
        original_private_expression_headers_v22()?,
    ])
}

impl SourceBoundaryCheckV31<'_> {
    fn normalize(
        &self,
        expression: &ProductionSemanticExpressionV2,
        actual: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        match self.optimized {
            None => source_scalar_expression_value_v18(
                self.leaves,
                self.arguments,
                self.origins,
                self.inline,
                expression,
                actual,
                budget,
            ),
            Some(leaves) => {
                let normalizer = OptimizedSourceScalarNormalizationV18 {
                    leaves,
                    arguments: self.arguments,
                };
                optimized_source_scalar_expression_endpoint_v18(
                    self.leaves,
                    leaves.optimized,
                    self.inventory,
                    leaves.function.coordinate,
                    &normalizer,
                    self.origins,
                    self.inline,
                    expression,
                    actual,
                    budget,
                )
            }
        }
    }

    fn placement(
        &self,
        input: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
        terminal: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1> {
        budget.charge_work(2)?;
        match self.optimized {
            None => Ok(input),
            Some(leaves) => {
                let original = self
                    .leaves
                    .relation
                    .source
                    .root(self.leaves.root, budget)?
                    .1;
                if input.function.0 as usize != original {
                    return self
                        .leaves
                        .relation
                        .source
                        .missing("source SSA block placement belongs to another root");
                }
                let rows = if terminal {
                    &leaves.boundaries.terminals
                } else {
                    &leaves.boundaries.entries
                };
                rows.get(input.block as usize).copied().flatten().ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "source SSA optimized boundary control was erased",
                    ),
                )
            }
        }
    }

    fn block(
        &self,
        input: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>> {
        let coordinate = self.placement(input, true, budget)?;
        let function = self
            .inventory
            .functions()
            .get(coordinate.function.0 as usize)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source SSA physical block function is absent",
            ))?;
        self.inventory
            .blocks()
            .get(
                function
                    .blocks
                    .start
                    .checked_add(coordinate.block as usize)
                    .ok_or(ArgumentResourceV1::Arithmetic)?,
            )
            .filter(|block| block.coordinate == coordinate)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source SSA physical block placement differs",
            ))
    }

    fn definition(
        &self,
        row: &SourceScalarBoundaryV31,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        match self.optimized {
            None => Ok(row.definition),
            Some(leaves) => {
                budget.charge_work(
                    self.leaves
                        .boundaries
                        .lookup
                        .len()
                        .checked_ilog2()
                        .unwrap_or(0) as usize
                        + 3,
                )?;
                let at = self
                    .leaves
                    .boundaries
                    .lookup
                    .binary_search_by_key(&[3, row.definition, 0, 0], |row| row.key)
                    .map_err(|_| {
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "source SSA optimized original boundary is absent",
                        )
                    })?;
                let definition = leaves
                    .boundaries
                    .definitions
                    .get(self.leaves.boundaries.lookup[at].row)
                    .copied()
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "source SSA optimized boundary definition is absent",
                    ))?;
                let value = self
                    .inventory
                    .definitions()
                    .get(definition)
                    .filter(|row| matches!(row.coordinate,
                        fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument { block, .. }
                        if block.function == leaves.function.coordinate))
                    .and_then(|definition| definition.value)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "source SSA optimized boundary definition is absent",
                    ))?;
                if !leaves
                    .boundary_value_v31(value, budget)?
                    .is_some_and(|original| std::ptr::eq(original, row))
                {
                    return self.leaves.relation.source.missing(
                        "source SSA optimized boundary definition names another original",
                    );
                }
                Ok(definition)
            }
        }
    }

    #[cfg(test)]
    fn boundary_target(
        &self,
        definition: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<bool> {
        Ok(self.boundary_name_v32(definition, budget)?.is_some())
    }

    fn boundary_name_v32(
        &self,
        definition: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<&SourceScalarBoundaryV31>> {
        budget.charge_work(4)?;
        let row = self.inventory.definitions().get(definition).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("source SSA argument definition is absent"),
        )?;
        let function = match self.optimized {
            Some(leaves) => leaves.function.coordinate,
            None => {
                let root = self.leaves.relation.source.root_row(self.leaves.root)?;
                self.inventory
                    .functions()
                    .get(root.function_ordinal)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "source SSA argument function is absent",
                    ))?
                    .coordinate
            }
        };
        if !matches!(row.coordinate,
            fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument { block, .. }
            if block.function == function)
        {
            return Ok(None);
        }
        let value = row.value.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source SSA argument value is absent",
        ))?;
        match self.optimized {
            None => {
                if let Some(name) = self
                    .leaves
                    .boundary_find_v31([3, definition, 0, 0], budget)?
                {
                    Ok(Some(name))
                } else {
                    self.leaves
                        .boundary_find_v31([4, value.0 as usize, 0, 0], budget)
                }
            }
            Some(leaves) => leaves.boundary_value_v31(value, budget),
        }
    }

    fn forwarded_edge_v32(
        &self,
        binding: &fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1,
        target: &SourceScalarBoundaryV31,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        budget.charge_work(5)?;
        let incoming = self
            .inventory
            .definitions()
            .get(binding.incoming_definition)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source SSA forwarded incoming definition is absent",
            ))?;
        let output = self
            .inventory
            .definitions()
            .get(binding.target_definition)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source SSA forwarded target definition is absent",
            ))?;
        if incoming.value != Some(binding.value)
            || incoming.ty != output.ty
            || !matches!(output.coordinate,
                fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument { block, argument }
                if block.function == binding.coordinate.edge.source.function && argument == binding.coordinate.argument)
            || !self
                .boundary_name_v32(binding.incoming_definition, budget)?
                .is_some_and(|source| std::ptr::eq(source, target))
        {
            return self
                .leaves
                .relation
                .source
                .missing("source SSA forwarded edge changes its checked boundary");
        }
        Ok(())
    }

    fn incoming_census_v32(
        &self,
        seen: &mut [bool],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        if seen.len() != self.bindings.len() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        for (at, binding) in self.inventory.edge_arguments()[self.bindings.clone()]
            .iter()
            .enumerate()
        {
            budget.charge_work(2)?;
            let Some(target) = self.boundary_name_v32(binding.target_definition, budget)? else {
                continue;
            };
            if self.definition(target, budget)? == binding.target_definition {
                if !seen[at] {
                    return self
                        .leaves
                        .relation
                        .source
                        .missing("source SSA boundary has an unaccounted incoming edge");
                }
            } else {
                // Only forwarding equations use the precomputed all-incoming
                // alias table. An opaque terminal still needs its source edge.
                if seen[at] {
                    return self
                        .leaves
                        .relation
                        .source
                        .missing("source SSA forwarded incoming edge repeats");
                }
                self.forwarded_edge_v32(binding, target, budget)?;
                seen[at] = true;
            }
        }
        Ok(())
    }
    fn expression(
        &self,
        instance: usize,
        variable: BoundaryVariableV31,
        value: EntryValueV20,
        ty: SemanticTypeIdV1,
        scalar: ProductionSemanticScalarTypeV2,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticExpressionV2> {
        let leaves = ProductionSourceScalarLeavesV18 {
            leaves: self.leaves,
        };
        if let EntryValueV20::BlockArgument {
            block,
            variable: original,
        } = value
        {
            if variable != original {
                return self
                    .leaves
                    .relation
                    .source
                    .missing("source SSA edge changes its local");
            }
            return leaves.boundary_expression_v31(instance, block, variable, ty, scalar, budget);
        }
        let (function_id, _) =
            self.leaves
                .relation
                .source
                .instance(self.leaves.root, instance, budget)?;
        let definition = self.index.definition(function_id, value, budget)?;
        if definition.local != variable.get() {
            return self
                .leaves
                .relation
                .source
                .missing("source SSA edge definition changes its local");
        }
        let function = leaves.original_function(instance, budget)?;
        let input = match definition.origin {
            OriginalEntryDefinitionV20::Argument(argument) => {
                OriginalPrivateInputV22::Argument(argument)
            }
            OriginalEntryDefinitionV20::Assignment { block, statement } => {
                let Some(SemanticStatementKindV1::Assign(assignment)) = function
                    .blocks()
                    .get(block as usize)
                    .and_then(|block| block.statements().get(statement as usize))
                    .map(|statement| statement.kind())
                else {
                    return self
                        .leaves
                        .relation
                        .source
                        .missing("source SSA edge assignment is absent");
                };
                if assignment.destination().local().index() != variable.get()
                    || !assignment.destination().projections().is_empty()
                    || assignment.destination().ty() != ty
                {
                    return self
                        .leaves
                        .relation
                        .source
                        .missing("source SSA edge assignment destination differs");
                }
                OriginalPrivateInputV22::Rvalue {
                    block,
                    statement,
                    value: assignment.value(),
                }
            }
        };
        let mut remaining = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
        self.index.private_expression_v22(
            &leaves,
            instance,
            ty,
            scalar,
            input,
            0,
            &mut remaining,
            budget,
        )
    }

    fn value(
        &self,
        row: &SourceScalarBoundaryV31,
        original: EntryValueV20,
        actual: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let run = |budget: &mut ArgumentBudgetV1<'_>| {
            self.leaves.relation.retain_query((|| {
                let expression = self.expression(
                    row.instance,
                    row.variable,
                    original,
                    row.ty,
                    row.scalar,
                    budget,
                )?;
                self.normalize(&expression, actual, budget)?;
                drop(expression);
                Ok(())
            })())
        };
        let headers = argument_sum_v1(&[
            std::mem::size_of_val(&run),
            std::mem::align_of_val(&run),
            size_of::<SourceOwnedResultV18<ProductionSemanticExpressionV2>>(),
        ])?;
        source_scalar_normalization_scratch_v18(
            self.leaves.relation.source.cleanup,
            budget,
            headers,
            run,
        )
    }

    fn selector(
        &self,
        instance: usize,
        block: SemanticBlockIdV1,
        source: &SemanticOperandV1,
        actual: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let run = |budget: &mut ArgumentBudgetV1<'_>| {
            self.leaves.relation.retain_query((|| {
                let owner = self.leaves.relation.source.source_semantic(budget)?;
                let ty = semantic_operand_type(source);
                let scalar = kir_semantic_scalar_v1(
                    &lower_scalar_type(owner.types(), ty).map_err(source_emission_error_v18)?,
                )
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source SSA branch selector is not scalar",
                ))?;
                let view = ProductionSourceScalarLeavesV18 {
                    leaves: self.leaves,
                };
                let mut remaining = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
                let expression = self.index.private_expression_v22(
                    &view,
                    instance,
                    ty,
                    scalar,
                    OriginalPrivateInputV22::Operand {
                        site: EntrySiteV20::Terminator {
                            block: BoundaryBlockV31::new(block.index()),
                        },
                        role: EntryOperandV20::SwitchDiscriminant,
                        operand: source,
                    },
                    0,
                    &mut remaining,
                    budget,
                )?;
                self.normalize(&expression, actual, budget)?;
                drop(expression);
                Ok(())
            })())
        };
        let headers = argument_sum_v1(&[
            std::mem::size_of_val(&run),
            std::mem::align_of_val(&run),
            size_of::<SourceOwnedResultV18<ProductionSemanticExpressionV2>>(),
        ])?;
        source_scalar_normalization_scratch_v18(
            self.leaves.relation.source.cleanup,
            budget,
            headers,
            run,
        )
    }

    fn edge(
        &self,
        source: &fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>,
        ordinal: usize,
        row: &SourceScalarBoundaryV31,
        original: EntryValueV20,
        seen: &mut [bool],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let relation = self.leaves.relation;
        budget.charge_work(8)?;
        let definition_index = self.definition(row, budget)?;
        let definition = &self.inventory.definitions()[definition_index];
        let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument { block, argument } =
            definition.coordinate
        else {
            return relation
                .source
                .missing("source SSA boundary is not a block argument");
        };
        let edge = self
            .inventory
            .edges()
            .get(
                source
                    .edges
                    .start
                    .checked_add(ordinal)
                    .ok_or(ArgumentResourceV1::Arithmetic)?,
            )
            .filter(|edge| {
                ordinal < source.edges.len()
                    && edge.coordinate.source == source.coordinate
                    && edge.coordinate.successor as usize == ordinal
            })
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source SSA ordered physical edge is absent",
            ))?;
        if edge.target != block
            || argument as usize >= edge.arguments.len()
            || edge.arguments.len() != edge.bindings.len()
        {
            return relation
                .source
                .missing("source SSA ordered physical target differs");
        }
        let binding_index = edge
            .bindings
            .start
            .checked_add(argument as usize)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let binding = self.inventory.edge_arguments().get(binding_index).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding(
                "source SSA physical argument binding is absent",
            ),
        )?;
        let seen_index = source_boundary_seen_index_v31(&self.bindings, binding_index)?;
        if binding.target_definition != definition_index
            || binding.coordinate.edge != edge.coordinate
            || binding.coordinate.argument != argument
            || binding.value != edge.arguments[argument as usize]
            || *seen.get(seen_index).ok_or(ArgumentResourceV1::Accounting)?
        {
            return relation
                .source
                .missing("source SSA physical argument binding differs or repeats");
        }
        self.value(row, original, binding.value, budget)?;
        seen[seen_index] = true;
        Ok(())
    }
}

impl SourceScalarLeavesV18<'_, '_> {
    fn check_boundary_equations_v31(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.check_boundary_actual_v31(None, budget)
    }

    fn check_boundary_actual_v31(
        &self,
        optimized: Option<&ProductionOptimizedSourceScalarLeavesV18<'_>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        if self.boundaries.rows.is_empty() {
            return Ok(());
        }
        let relation = self.relation;
        let run = |budget: &mut ArgumentBudgetV1<'_>| {
            relation.retain_query((|| {
                let index = OriginalEntryIndexV20::build(relation, budget)?;
                let arguments = SourceRootArgumentsV18::build(relation, self.root, budget)?;
                let inline = Gfx942InlineScalarCorrespondenceV30::build_source_v18(
                    relation, self.root, budget,
                )?;
                let output_inline = optimized
                    .map(|leaves| {
                        inline.transport_optimized_source_v18(
                            relation,
                            leaves.optimized,
                            self.root,
                            budget,
                        )
                    })
                    .transpose()?;
                let inventory = match optimized {
                    Some(leaves) => leaves.optimized.output_inventory(budget)?,
                    None => relation.inventory,
                };
                let root = relation.source.root_row(self.root)?;
                let controls = source_reference_selection_control_index_v30(
                    &root.coordinates.controls.rows,
                    budget,
                )
                .map_err(source_emission_error_v18)?;
                let function = relation
                    .inventory
                    .functions()
                    .get(root.function_ordinal)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "source SSA boundary physical function is absent",
                    ))?;
                let (bindings, mut seen) = source_boundary_function_seen_v31(
                    inventory,
                    optimized.map_or(function.coordinate, |leaves| leaves.function.coordinate),
                    budget,
                )?;
                let nested = |origins: &value_origin_v1::WholeValueOriginsV18<'_>,
                              budget: &mut ArgumentBudgetV1<'_>| {
                    relation.retain_query((|| {
                        let check = SourceBoundaryCheckV31 {
                            leaves: self,
                            index: &index,
                            arguments: &arguments,
                            origins,
                            inline: output_inline.as_ref().unwrap_or(&inline),
                            optimized,
                            inventory,
                            bindings: bindings.clone(),
                        };
                        for source in &root.coordinates.sources.rows {
                            budget.charge_work(
                                self.boundaries.lookup.len().checked_ilog2().unwrap_or(0) as usize
                                    + 2,
                            )?;
                            let instance = source.instance.index();
                            let first = self
                                .boundaries
                                .lookup
                                .partition_point(|row| row.key < [0, instance, 0, 0]);
                            if self
                                .boundaries
                                .lookup
                                .get(first)
                                .is_none_or(|row| row.key[0] != 0 || row.key[1] != instance)
                            {
                                continue;
                            }
                            let owner = relation.source.source_ssa(budget)?;
                            let declaration = owner
                                .source_semantic()
                                .functions()
                                .get(source.function.index() as usize)
                                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                    "source SSA boundary declaration is absent",
                                ))?;
                            let plan = owner
                                .plan_for_function(source.function)
                                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                    "source SSA boundary plan is absent",
                                ))?
                                .plan();
                            for block in plan.reverse_postorder() {
                                let semantic_block = SemanticBlockIdV1::from_index(block.get());
                                let original = declaration
                                    .blocks()
                                    .get(block.get() as usize)
                                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                        "source SSA boundary source block is absent",
                                    ))?
                                    .terminator()
                                    .kind();
                                budget.charge_work(
                                    controls.len().checked_ilog2().unwrap_or(0) as usize + 5,
                                )?;
                                let control = &controls[controls
                                    .binary_search_by_key(&(instance, block.get()), |row| {
                                        (row.instance.index(), row.source.index())
                                    })
                                    .map_err(|_| {
                                        ProductionSourceOwnedViewErrorV18::Binding(
                                            "source SSA boundary control exit is absent",
                                        )
                                    })?];
                                let original_actual = relation
                                    .inventory
                                    .block_for_id(function.coordinate, control.terminal, budget)
                                    .map_err(source_pointer_inventory_error_v18)?
                                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                        "source SSA boundary physical exit is absent",
                                    ))?;
                                let actual = check.block(original_actual.coordinate, budget)?;
                                let entry = relation
                                    .source_block_entry(
                                        self.root,
                                        instance,
                                        semantic_block,
                                        budget,
                                    )?
                                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                        "source SSA boundary physical entry is absent",
                                    ))?;
                                let entry_row = relation
                                    .inventory
                                    .block_for_id(function.coordinate, control.entry, budget)
                                    .map_err(source_pointer_inventory_error_v18)?
                                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                        "source SSA boundary control entry is absent",
                                    ))?;
                                if entry_row.coordinate != entry {
                                    return relation
                                        .source
                                        .missing("source SSA boundary control entry differs");
                                }
                                match (original, actual.terminator) {
                                    (
                                        SemanticTerminatorKindV1::Goto(_),
                                        Terminator::Branch { .. },
                                    ) => {}
                                    (
                                        SemanticTerminatorKindV1::Call(call),
                                        Terminator::Branch { .. },
                                    ) if call.destination().is_some()
                                        && original.edge_count() == 1 => {}
                                    (
                                        SemanticTerminatorKindV1::SwitchInt {
                                            discriminant,
                                            targets,
                                        },
                                        Terminator::Switch {
                                            selector, cases, ..
                                        },
                                    ) => {
                                        budget.charge_work(cases.len())?;
                                        if targets.values().len() != cases.len()
                                            || !targets
                                                .values()
                                                .iter()
                                                .zip(cases)
                                                .all(|(a, b)| a.value() == u128::from(b.value))
                                        {
                                            return relation
                                                .source
                                                .missing("source SSA branch case values differ");
                                        }
                                        check.selector(
                                            instance,
                                            semantic_block,
                                            discriminant,
                                            *selector,
                                            budget,
                                        )?;
                                    }
                                    (
                                        SemanticTerminatorKindV1::SwitchInt {
                                            discriminant,
                                            targets,
                                        },
                                        Terminator::ConditionalBranch { condition, .. },
                                    ) => {
                                        if lower_scalar_type(
                                            owner.source_semantic().types(),
                                            discriminant.ty(),
                                        )
                                        .map_err(source_emission_error_v18)?
                                            != Type::BOOL
                                            || targets.values().len() != 1
                                            || targets.values()[0].value() > 1
                                        {
                                            return relation
                                                .source
                                                .missing("source SSA Boolean branch case differs");
                                        }
                                        check.selector(
                                            instance,
                                            semantic_block,
                                            discriminant,
                                            *condition,
                                            budget,
                                        )?;
                                    }
                                    (
                                        SemanticTerminatorKindV1::Return,
                                        Terminator::Return { .. } | Terminator::Branch { .. },
                                    )
                                    | (
                                        SemanticTerminatorKindV1::Unreachable,
                                        Terminator::Unreachable,
                                    ) => {}
                                    _ => {
                                        return relation.source.missing(
                                            "source SSA boundary control contract is not modeled",
                                        );
                                    }
                                }
                                let mut ordinal = 0usize;
                                original.try_for_each_edge(|edge| {
                                    let target = BoundaryBlockV31::new(edge.target().index());
                                    let physical = source_reference_selection_edge_ordinal_v30(
                                        owner.source_semantic().types(),
                                        original,
                                        edge.role(),
                                        ordinal,
                                        actual.terminator,
                                        budget,
                                    )
                                    .map_err(source_emission_error_v18)?;
                                    let target_entry = relation
                                        .source_block_entry(
                                            self.root,
                                            instance,
                                            edge.target(),
                                            budget,
                                        )?
                                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                            "source SSA edge target is absent",
                                        ))?;
                                    let actual_edge = inventory
                                        .edges()
                                        .get(
                                            actual
                                                .edges
                                                .start
                                                .checked_add(physical)
                                                .ok_or(ArgumentResourceV1::Arithmetic)?,
                                        )
                                        .filter(|_| physical < actual.edges.len())
                                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                            "source SSA ordered edge is absent",
                                        ))?;
                                    if actual_edge.target
                                        != check.placement(target_entry, false, budget)?
                                    {
                                        return relation
                                            .source
                                            .missing("source SSA edge target changed");
                                    }
                                    let arguments = plan
                                        .edge_arguments(BoundaryEdgeV31::new(
                                            *block,
                                            u32::try_from(ordinal)
                                                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                                        ))
                                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                            "source SSA edge arguments are absent",
                                        ))?;
                                    for argument in arguments {
                                        budget.charge_work(1)?;
                                        if let Some(row) = self.boundary_find_v31(
                                            [
                                                0,
                                                instance,
                                                target.get() as usize,
                                                argument.variable().get() as usize,
                                            ],
                                            budget,
                                        )? {
                                            check.edge(
                                                actual,
                                                physical,
                                                row,
                                                argument.value(),
                                                &mut seen,
                                                budget,
                                            )?;
                                        }
                                    }
                                    ordinal = ordinal
                                        .checked_add(1)
                                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                                })?;
                            }
                            self.check_boundary_invocation_v31(
                                &check,
                                source.instance,
                                source.function,
                                declaration,
                                plan,
                                &mut seen,
                                budget,
                            )?;
                        }
                        check.incoming_census_v32(&mut seen, budget)
                    })())
                };
                let capture = std::mem::size_of_val(&nested);
                budget.reserve_storage(argument_sum_v1(&[
                    capture,
                    std::mem::align_of_val(&nested),
                ])?)?;
                match optimized {
                    None => value_origin_v1::with_whole_value_origins_v18(
                        relation,
                        function.coordinate,
                        budget,
                        nested,
                    )?,
                    Some(leaves) => value_origin_v1::with_optimized_whole_value_origins_v18(
                        relation,
                        leaves.optimized,
                        inventory,
                        leaves.function.coordinate,
                        budget,
                        nested,
                    )?,
                }
                drop((seen, controls, output_inline, inline, arguments, index));
                Ok(())
            })())
        };
        let headers = argument_sum_v1(&[
            source_boundary_control_headers_v31()?,
            std::mem::size_of_val(&run),
            std::mem::align_of_val(&run),
        ])?;
        source_scalar_normalization_scratch_v18(relation.source.cleanup, budget, headers, run)
    }

    fn check_boundary_invocation_v31(
        &self,
        check: &SourceBoundaryCheckV31<'_>,
        instance: ProductionCallInstanceIdV1,
        function_id: SemanticFunctionIdV1,
        declaration: &SemanticFunctionDeclV1,
        plan: &fe2o3_mir_model::SsaConstructionPlanV1,
        seen: &mut [bool],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let relation = self.relation;
        let sidecar = relation
            .source
            .sidecar(self.root, instance.index(), budget)?;
        for argument in plan.entry_arguments() {
            budget.charge_work(1)?;
            let Some(row) = self.boundary_find_v31(
                [
                    0,
                    instance.index(),
                    declaration.entry().index() as usize,
                    argument.variable().get() as usize,
                ],
                budget,
            )?
            else {
                continue;
            };
            let entry = sidecar.invocation_entry.as_ref().ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("source SSA invocation entry is absent"),
            )?;
            budget.charge_work(entry.arguments.len().checked_ilog2().unwrap_or(0) as usize + 9)?;
            let at = entry
                .arguments
                .binary_search_by_key(&argument.variable(), |row| row.original.variable())
                .map_err(|_| {
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "source SSA invocation argument is absent",
                    )
                })?;
            let input = entry.arguments[at];
            let component = entry.components.get(input.first_component).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "source SSA invocation component is absent",
                ),
            )?;
            let original = relation
                .ssa_scalar_definition_v30(self.root, instance.index(), argument.value(), budget)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source SSA invocation original is not scalar",
                ))?;
            if entry.span.version != INVOCATION_ENTRY_RELATION_VERSION_V1
                || entry.span.semantic_function != function_id
                || input.original != *argument
                || input.component_count != 1
                || component.parameter != row.value
                || relation.inventory.definitions()[original].value != Some(component.original)
                || component.transported != component.original
                || component.conversion.is_some()
            {
                return relation
                    .source
                    .missing("source SSA invocation input differs from original entry");
            }
            let preheader =
                entry
                    .layout
                    .preheader
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "source SSA invocation preheader is absent",
                    ))?;
            let gap = entry
                .span
                .first_operation_ordinal
                .checked_add(entry.span.operation_count)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            let root = relation.source.root_row(self.root)?;
            let mut mapping = ScopedEmittedPointsV29 {
                coordinates: &root.coordinates,
                relocation: &root.slot_relocation,
                budget,
            };
            let (source, position) = mapping
                .emitted_point(instance, preheader, gap, true)
                .map_err(|error| source_emission_error_v18(source_address_point_error_v29(error)))?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source SSA invocation relocation is absent",
                ))?;
            let physical = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                u32::try_from(root.function_ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            let source = relation
                .inventory
                .block_for_id(physical, source, budget)
                .map_err(source_pointer_inventory_error_v18)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source SSA invocation physical block is absent",
                ))?;
            if position as usize != source.block.operations.len()
                || !matches!(source.terminator, Terminator::Branch { .. })
                || source.edges.len() != 1
            {
                return relation
                    .source
                    .missing("source SSA invocation branch differs");
            }
            let actual = check.block(source.coordinate, budget)?;
            check.edge(actual, 0, row, argument.value(), seen, budget)?;
        }
        Ok(())
    }
}
