#[derive(Clone, Copy)]
struct OptimizedSourceScalarBoundaryV31 {
    value: ValueId,
    original: usize,
    definition: usize,
}
struct OptimizedSourceScalarBoundariesV31 {
    values: Vec<OptimizedSourceScalarBoundaryV31>,
    definitions: Vec<usize>,
    entries: Vec<Option<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>>,
    terminals: Vec<Option<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>>,
}

struct OptimizedScalarCfgMeterV31<'a, 'g, 'b, 'w> {
    source: &'a ProductionSourceOwnedViewV18<'g>,
    budget: &'b mut ArgumentBudgetV1<'w>,
}
impl fe2o3_mir_model::SemanticAssertionMeterV1 for OptimizedScalarCfgMeterV31<'_, '_, '_, '_> {
    type Error = ProductionSourceOwnedViewErrorV18;
    fn charge_work(&mut self, work: usize) -> SourceOwnedResultV18<()> {
        self.source.check_query_v18(self.budget)?;
        self.budget
            .charge_work(work)
            .map_err(|error| self.source.retain_query_resource_error_v18(error))
    }
    fn reserve_storage(&mut self, bytes: usize) -> SourceOwnedResultV18<()> {
        self.source.check_query_v18(self.budget)?;
        self.budget
            .reserve_storage(bytes)
            .map_err(|error| self.source.retain_query_resource_error_v18(error))
    }
}

fn optimized_source_boundary_headers_v31() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        OptimizedSourceScalarBoundariesV31,
        OptimizedSourceScalarBoundaryV31,
        Vec<OptimizedSourceScalarBoundaryV31>,
        Vec<usize>,
        [Vec<Option<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>>; 2],
        Result<Vec<OptimizedSourceScalarBoundaryV31>, ProductionSemanticKirErrorV1>,
        Result<Vec<usize>, ProductionSemanticKirErrorV1>,
        [Result<
            Vec<Option<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>>,
            ProductionSemanticKirErrorV1,
        >; 2],
        SourceOwnedResultV18<OptimizedSourceScalarBoundariesV31>,
        SourceOwnedResultV18<Option<&'a SourceScalarBoundaryV31>>,
        ProductionOptimizedSourceCfgRootV18<'a, 'a>,
        SourceOwnedResultV18<ProductionOptimizedSourceCfgRootV18<'a, 'a>>,
        OptimizedScalarCfgMeterV31<'a, 'a, 'a, 'a>,
        Option<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>,
        &'a SourceScalarBoundaryV31,
        &'a [fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1],
        [usize; 8],
        [SourceOwnedResultV18<()>; 2],
    );
    argument_sum_v1(&[size_of::<Frame<'_>>(), std::mem::align_of::<Frame<'_>>()])
}

fn optimized_source_scalar_boundaries_v31(
    original: &ProductionSourceScalarLeavesV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<OptimizedSourceScalarBoundariesV31> {
    let leaves = original.leaves;
    let relation = leaves.relation;
    relation.retain_query((|| {
        if leaves.boundaries.rows.is_empty() {
            return Ok(OptimizedSourceScalarBoundariesV31 {
                values: Vec::new(),
                definitions: Vec::new(),
                entries: Vec::new(),
                terminals: Vec::new(),
            });
        }
        optimized_source_endpoints_v18(relation, optimized, budget)?;
        let cfg = optimized.output_root_cfg_v18(leaves.root, budget)?;
        let input = &relation.inventory.functions()
            [relation.source.root_row(leaves.root)?.function_ordinal];
        let output = cfg.inventory();
        let mut terminals =
            emission_vec_v1(input.blocks.len(), budget).map_err(source_emission_error_v18)?;
        budget.charge_work(input.blocks.len())?;
        terminals.resize(input.blocks.len(), None);
        let mut entries =
            emission_vec_v1(input.blocks.len(), budget).map_err(source_emission_error_v18)?;
        budget.charge_work(input.blocks.len())?;
        entries.resize(input.blocks.len(), None);
        cfg.visit(
            &mut OptimizedScalarCfgMeterV31 {
                source: relation.source,
                budget,
            },
            |event, meter| {
                if let ProductionOptimizedSourceCfgEventV18::Block { actual, segments } = event {
                    for (at, segment) in segments.iter().enumerate() {
                        meter.charge_work(4)?;
                        if segment.input.function != input.coordinate {
                            return relation
                                .source
                                .missing("source SSA optimized block belongs to another function");
                        }
                        let slot = entries.get_mut(segment.input.block as usize).ok_or(
                            ProductionSourceOwnedViewErrorV18::Binding(
                                "source SSA optimized entry locator differs",
                            ),
                        )?;
                        if slot.replace(actual.coordinate).is_some() {
                            return relation
                                .source
                                .missing("source SSA optimized entry repeats");
                        }
                        if at + 1 == segments.len() {
                            let slot = terminals.get_mut(segment.input.block as usize).ok_or(
                                ProductionSourceOwnedViewErrorV18::Binding(
                                    "source SSA optimized block locator differs",
                                ),
                            )?;
                            if segment.connector.is_some()
                                || slot.replace(actual.coordinate).is_some()
                            {
                                return relation
                                    .source
                                    .missing("source SSA optimized terminal block repeats");
                            }
                        }
                    }
                }
                Ok(())
            },
        )?;
        let function = cfg.function();
        let mut values = emission_vec_v1(function.definitions.len(), budget)
            .map_err(source_emission_error_v18)?;
        let mut definitions = emission_vec_v1(leaves.boundaries.rows.len(), budget)
            .map_err(source_emission_error_v18)?;
        for (original, row) in leaves.boundaries.rows.iter().enumerate() {
            let input = relation.inventory.definitions()[row.definition].coordinate;
            let mut retained = None;
            for descendant in optimized.definition_descendants(input, budget)? {
                budget.charge_work(4)?;
                if descendant.kind
                    != fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1::Retained
                {
                    continue;
                }
                let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument {
                    block,
                    argument,
                } = descendant.output
                else {
                    return relation.source.missing(
                        "source SSA optimized boundary is not a retained block parameter",
                    );
                };
                let function = output.functions().get(block.function.0 as usize).ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "source SSA optimized boundary function is absent",
                    ),
                )?;
                let block = output
                    .blocks()
                    .get(
                        function
                            .blocks
                            .start
                            .checked_add(block.block as usize)
                            .ok_or(ArgumentResourceV1::Arithmetic)?,
                    )
                    .filter(|actual| actual.coordinate == block)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "source SSA optimized boundary block is absent",
                    ))?;
                let at = block
                    .parameters
                    .start
                    .checked_add(argument as usize)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                let definition = output
                    .definitions()
                    .get(at)
                    .filter(|_| (argument as usize) < block.parameters.len())
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "source SSA optimized boundary parameter is absent",
                    ))?;
                if definition.coordinate != descendant.output
                    || kir_semantic_scalar_v1(definition.ty) != Some(row.scalar)
                    || retained.replace((at, definition.value)).is_some()
                {
                    return relation.source.missing(
                        "source SSA optimized boundary type or retained identity differs",
                    );
                }
            }
            let (definition, Some(value)) =
                retained.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source SSA optimized boundary has no retained parameter",
                ))?
            else {
                return relation
                    .source
                    .missing("source SSA optimized boundary value is absent");
            };
            definitions.push(definition);
            values.push(OptimizedSourceScalarBoundaryV31 {
                value,
                original,
                definition,
            });
        }
        source_scalar_normalization_scratch_v18(
            relation.source.cleanup,
            budget,
            source_boundary_forwarding_headers_v32()?,
            |budget| {
                let mut terminals =
                    emission_vec_v1(values.len(), budget).map_err(source_emission_error_v18)?;
                for row in &values {
                    budget.charge_work(1)?;
                    terminals.push((row.definition, row.original));
                }
                let forwarded = source_boundary_forwarding_v32(
                    output,
                    function.coordinate,
                    &terminals,
                    budget,
                )?;
                for row in &forwarded {
                    budget.charge_work(2)?;
                    if values.len() == values.capacity() {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    values.push(*row);
                }
                drop((forwarded, terminals));
                Ok(())
            },
        )?;
        private_array_heapsort_v1(
            &mut values,
            |row| [row.value.0 as usize],
            &mut SourceCorrespondenceWorkV18(budget),
            || ArgumentResourceV1::Arithmetic.into(),
        )?;
        for pair in values.windows(2) {
            budget.charge_work(1)?;
            if pair[0].value == pair[1].value {
                return relation
                    .source
                    .missing("source SSA optimized boundaries were conflated");
            }
        }
        Ok(OptimizedSourceScalarBoundariesV31 {
            values,
            definitions,
            entries,
            terminals,
        })
    })())
}

impl ProductionOptimizedSourceScalarLeavesV18<'_> {
    fn boundary_value_v31(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<&SourceScalarBoundaryV31>> {
        let relation = self.original.leaves.relation;
        relation.retain_query((|| {
            self.check(budget)?;
            budget.charge_work(self.boundaries.values.len().checked_ilog2().unwrap_or(0) as usize + 4)?;
            let Ok(at) = self.boundaries.values.binary_search_by_key(&value, |row| row.value) else { return Ok(None); };
            let row = self.boundaries.values[at];
            let original = self.original.leaves.boundaries.rows.get(row.original).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("source SSA optimized original row is absent"))?;
            let output = self.optimized.output_inventory(budget)?;
            let definition = output.definitions().get(row.definition).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("source SSA optimized definition is absent"))?;
            if definition.value != Some(value) || kir_semantic_scalar_v1(definition.ty) != Some(original.scalar)
                || !matches!(definition.coordinate, fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument { block, .. } if block.function == self.function.coordinate) {
                return relation.source.missing("source SSA optimized parameter changed");
            }
            Ok(Some(original))
        })())
    }
}
