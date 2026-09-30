type AggregateOperationV30 = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1;
type AggregateEdgeV30 = fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1;
type AggregateFunctionV30 = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1;

#[derive(Clone, Copy)]
enum AggregateUniqueCoordinateV30<T> {
    Missing,
    One(T),
    Multiple,
}

struct AggregateCoordinateTransportV30 {
    definitions: AggregateDefinitionTransportV30,
    operations: Vec<AggregateOperationV30>,
    edges: Vec<AggregateEdgeV30>,
    functions: Vec<AggregateFunctionV30>,
}

impl AggregateStageStateV30 for AggregateCoordinateTransportV30 {
    fn retained_storage(&self) -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            self.definitions.retained_storage()?,
            argument_product_v1(
                self.operations.capacity(),
                size_of::<AggregateOperationV30>(),
            )?,
            argument_product_v1(self.edges.capacity(), size_of::<AggregateEdgeV30>())?,
            argument_product_v1(self.functions.capacity(), size_of::<AggregateFunctionV30>())?,
        ])
    }
}

fn aggregate_coordinate_headers_v30() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        AggregateCoordinateTransportV30,
        Result<AggregateCoordinateTransportV30, ProductionAggregateSourceErrorV30>,
        AggregateDefinitionTransportV30,
        Result<AggregateDefinitionTransportV30, ProductionAggregateSourceErrorV30>,
        [&'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>; 2],
        &'a AggregateSourceStageV30<'a>,
        &'a mut ArgumentBudgetV1<'a>,
        Vec<AggregateUniqueCoordinateV30<AggregateOperationV30>>,
        Vec<AggregateUniqueCoordinateV30<AggregateEdgeV30>>,
        Vec<AggregateUniqueCoordinateV30<AggregateFunctionV30>>,
        [AggregateOperationV30; 3],
        [AggregateEdgeV30; 3],
        [Option<usize>; 3],
        [usize; 16],
        Result<usize, ProductionSourceOwnedViewErrorV18>,
        &'a fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'a>,
        &'a fe2o3_kernel_ir::CanonicalKirOperationTransitionV1,
        &'a fe2o3_kernel_ir::CanonicalKirEdgeTransitionV1,
        Option<fe2o3_kernel_analysis::CanonicalKirAggregateOperationTransportV30>,
        Result<
            Option<fe2o3_kernel_analysis::CanonicalKirAggregateOperationTransportV30>,
            fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18,
        >,
    );
    argument_sum_v1(&[size_of::<Frame<'_>>(), std::mem::align_of::<Frame<'_>>()])
}

fn aggregate_operation_index_v30(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    coordinate: AggregateOperationV30,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    let block = source_block_row_v18(inventory, coordinate.block, budget)?;
    budget.charge_work(3)?;
    block
        .operations
        .start
        .checked_add(coordinate.operation as usize)
        .filter(|index| *index < block.operations.end)
        .filter(|index| inventory.operations()[*index].coordinate == coordinate)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "aggregate operation coordinate",
        ))
}

fn aggregate_edge_index_v30(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    coordinate: AggregateEdgeV30,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    let block = source_block_row_v18(inventory, coordinate.source, budget)?;
    budget.charge_work(3)?;
    block
        .edges
        .start
        .checked_add(coordinate.successor as usize)
        .filter(|index| *index < block.edges.end)
        .filter(|index| inventory.edges()[*index].coordinate == coordinate)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "aggregate successor coordinate",
        ))
}

fn aggregate_coordinate_table_v30<T: Copy>(
    count: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<AggregateUniqueCoordinateV30<T>>> {
    let mut table =
        source_reference_emission_vec_v29(count, budget).map_err(source_argument_error_v18)?;
    budget.charge_work(count)?;
    table.resize(count, AggregateUniqueCoordinateV30::Missing);
    Ok(table)
}

fn aggregate_record_coordinate_v30<T: Copy>(slot: &mut AggregateUniqueCoordinateV30<T>, value: T) {
    *slot = match *slot {
        AggregateUniqueCoordinateV30::Missing => AggregateUniqueCoordinateV30::One(value),
        _ => AggregateUniqueCoordinateV30::Multiple,
    };
}

impl AggregateCoordinateTransportV30 {
    fn seed(
        inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        definitions: &[fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1],
        operations: &[AggregateOperationV30],
        edges: &[AggregateEdgeV30],
        functions: &[AggregateFunctionV30],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Self> {
        let definitions = AggregateDefinitionTransportV30::seed(inventory, definitions, budget)?;
        let mut output_operations = source_reference_emission_vec_v29(operations.len(), budget)
            .map_err(source_argument_error_v18)?;
        for &operation in operations {
            aggregate_operation_index_v30(inventory, operation, budget)?;
            output_operations.push(operation);
        }
        let mut output_edges = source_reference_emission_vec_v29(edges.len(), budget)
            .map_err(source_argument_error_v18)?;
        for &edge in edges {
            aggregate_edge_index_v30(inventory, edge, budget)?;
            output_edges.push(edge);
        }
        let mut output_functions = source_reference_emission_vec_v29(functions.len(), budget)
            .map_err(source_argument_error_v18)?;
        for &function in functions {
            budget.charge_work(2)?;
            if inventory
                .functions()
                .get(function.0 as usize)
                .is_none_or(|row| row.coordinate != function)
            {
                return Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "aggregate original runtime root function",
                ));
            }
            output_functions.push(function);
        }
        Ok(Self {
            definitions,
            operations: output_operations,
            edges: output_edges,
            functions: output_functions,
        })
    }

    fn advance(
        mut self,
        stage: &AggregateSourceStageV30<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionAggregateSourceErrorV30> {
        use ProductionAggregateSourceErrorV30 as E;
        stage.check(budget)?;
        // Exact owner/stage currentness is also checked by definition advance;
        // do it before changing any of the owned occurrence rows.
        if self.definitions.owner != std::ptr::from_ref(stage.input.owner()) as usize
            || self.definitions.next_stage != stage.ordinal
        {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "aggregate coordinate transport owner or stage order",
            )
            .into());
        }
        match stage.relation {
            AggregateSourceStageRelationV30::Scalar(pair) => {
                let mut operations =
                    aggregate_coordinate_table_v30(stage.input.operations().len(), budget)?;
                let mut edges = aggregate_coordinate_table_v30(stage.input.edges().len(), budget)?;
                let mut functions =
                    aggregate_coordinate_table_v30(stage.input.functions().len(), budget)?;
                for row in pair.rows().functions {
                    budget.charge_work(2)?;
                    let slot = functions.get_mut(row.input.0 as usize).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "aggregate scalar function coordinate",
                        ),
                    )?;
                    aggregate_record_coordinate_v30(slot, row.output);
                }
                for function in &mut self.functions {
                    budget.charge_work(2)?;
                    let Some(AggregateUniqueCoordinateV30::One(output)) =
                        functions.get(function.0 as usize)
                    else {
                        return Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "aggregate runtime root has no unique scalar function",
                        )
                        .into());
                    };
                    *function = *output;
                }
                for row in pair.rows().operations {
                    budget.charge_work(2)?;
                    if let fe2o3_kernel_ir::CanonicalKirOperationOriginV1::Retained(input) =
                        row.origin
                    {
                        let slot = aggregate_operation_index_v30(stage.input, input, budget)?;
                        aggregate_record_coordinate_v30(&mut operations[slot], row.output);
                    }
                }
                for row in pair.rows().edges {
                    budget.charge_work(2)?;
                    let slot = aggregate_edge_index_v30(stage.input, row.input, budget)?;
                    aggregate_record_coordinate_v30(&mut edges[slot], row.output);
                }
                for operation in &mut self.operations {
                    let slot = aggregate_operation_index_v30(stage.input, *operation, budget)?;
                    let AggregateUniqueCoordinateV30::One(output) = operations[slot] else {
                        return Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "aggregate runtime effect has no unique retained scalar occurrence",
                        )
                        .into());
                    };
                    *operation = output;
                }
                for edge in &mut self.edges {
                    let slot = aggregate_edge_index_v30(stage.input, *edge, budget)?;
                    let AggregateUniqueCoordinateV30::One(output) = edges[slot] else {
                        return Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "aggregate runtime guard has no unique retained scalar successor",
                        )
                        .into());
                    };
                    *edge = output;
                }
                let credit = argument_sum_v1(&[
                    argument_product_v1(
                        functions.capacity(),
                        size_of::<AggregateUniqueCoordinateV30<AggregateFunctionV30>>(),
                    )?,
                    argument_product_v1(
                        operations.capacity(),
                        size_of::<AggregateUniqueCoordinateV30<AggregateOperationV30>>(),
                    )?,
                    argument_product_v1(
                        edges.capacity(),
                        size_of::<AggregateUniqueCoordinateV30<AggregateEdgeV30>>(),
                    )?,
                ])?;
                drop(edges);
                drop(functions);
                drop(operations);
                budget.release_storage(credit)?;
            }
            AggregateSourceStageRelationV30::Aggregate(_) => {
                let index =
                    stage
                        .aggregate_index
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "aggregate occurrence index absent",
                        ))?;
                let error = |error| {
                    E::Optimization(
                        fe2o3_kernel_opt::OwnedAggregateFixedpointErrorV18::Aggregate(
                            fe2o3_kernel_opt::OwnedAggregateSsaErrorV18::Check(error),
                        ),
                    )
                };
                for operation in &mut self.operations {
                    let slot = aggregate_operation_index_v30(stage.input, *operation, budget)?;
                    let retained = index
                        .operation(slot, budget)
                        .map_err(error)?
                        .filter(|row| row.is_retained())
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "aggregate runtime effect was removed or replaced",
                        ))?;
                    *operation = retained.output();
                }
                for edge in &mut self.edges {
                    let slot = aggregate_edge_index_v30(stage.input, *edge, budget)?;
                    *edge = index.edge(slot, budget).map_err(error)?;
                }
            }
        }
        self.definitions = self.definitions.advance(stage, budget)?;
        Ok(self)
    }
}
