// A dense, bounded relation carries only requested external definitions. It
// preserves every checked descendant; fresh final facts choose the actual value.
// Selecting the first descendant would lose substituted/retained distinctions.
struct AggregateDefinitionTransportV30 {
    owner: usize,
    next_stage: usize,
    rows: usize,
    columns: usize,
    relation: Vec<u8>,
}

fn aggregate_definition_headers_v30() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        AggregateDefinitionTransportV30,
        Vec<u8>,
        Result<AggregateDefinitionTransportV30, ProductionAggregateSourceErrorV30>,
        SourceOwnedResultV18<Vec<u8>>,
        SourceOwnedResultV18<usize>,
        SourceOwnedResultV18<bool>,
        [&'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>; 2],
        &'a [fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1],
        &'a AggregateSourceStageV30<'a>,
        &'a mut ArgumentBudgetV1<'a>,
        std::ops::Range<usize>,
        [fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1; 2],
        [usize; 16],
        [Option<usize>; 4],
    );
    argument_sum_v1(&[size_of::<Frame<'_>>(), std::mem::align_of::<Frame<'_>>()])
}

impl AggregateStageStateV30 for AggregateDefinitionTransportV30 {
    fn retained_storage(&self) -> Result<usize, ArgumentResourceV1> {
        Ok(self.relation.capacity())
    }
}

fn aggregate_definition_index_v30(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    coordinate: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as D;
    budget.charge_work(4)?;
    let (range, ordinal) = match coordinate {
        D::FunctionArgument { function, argument } => {
            let row = inventory
                .functions()
                .get(function.0 as usize)
                .filter(|row| row.coordinate == function)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "aggregate definition function",
                ))?;
            (row.definitions.clone(), argument)
        }
        D::BlockArgument { block, argument } => (
            source_block_row_v18(inventory, block, budget)?
                .parameters
                .clone(),
            argument,
        ),
        D::Result { operation, result } => (
            source_operation_row_v18(inventory, operation, budget)?
                .results
                .clone(),
            result,
        ),
    };
    let index = range
        .start
        .checked_add(ordinal as usize)
        .filter(|index| *index < range.end)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "aggregate definition ordinal",
        ))?;
    if inventory
        .definitions()
        .get(index)
        .is_none_or(|row| row.coordinate != coordinate)
    {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "aggregate definition coordinate",
        ));
    }
    Ok(index)
}

fn aggregate_relation_bytes_v30(
    count: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<u8>> {
    budget.charge_work(count)?;
    budget.reserve_storage(count)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(count)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    budget.reserve_storage(
        bytes
            .capacity()
            .checked_sub(count)
            .ok_or(ArgumentResourceV1::Accounting)?,
    )?;
    bytes.resize(count, 0);
    Ok(bytes)
}

impl AggregateDefinitionTransportV30 {
    fn seed(
        inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        definitions: &[fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Self> {
        let rows = inventory.definitions().len();
        let columns = definitions.len();
        let count = argument_product_v1(rows, columns)?;
        let mut relation = aggregate_relation_bytes_v30(count, budget)?;
        for (column, &definition) in definitions.iter().enumerate() {
            let row = aggregate_definition_index_v30(inventory, definition, budget)?;
            relation[row * columns + column] = 1;
        }
        Ok(Self {
            owner: std::ptr::from_ref(inventory.owner()) as usize,
            next_stage: 0,
            rows,
            columns,
            relation,
        })
    }

    fn advance(
        self,
        stage: &AggregateSourceStageV30<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionAggregateSourceErrorV30> {
        use ProductionAggregateSourceErrorV30 as E;
        stage.check(budget)?;
        budget.charge_work(5)?;
        if self.owner != std::ptr::from_ref(stage.input.owner()) as usize
            || self.next_stage != stage.ordinal
            || self.rows != stage.input.definitions().len()
            || self.relation.len() != argument_product_v1(self.rows, self.columns)?
        {
            return Err(stage
                .source
                .missing::<()>("aggregate definition transport owner or stage order")
                .unwrap_err()
                .into());
        }
        let rows = stage.output.definitions().len();
        let mut next =
            aggregate_relation_bytes_v30(argument_product_v1(rows, self.columns)?, budget)?;
        match stage.relation {
            AggregateSourceStageRelationV30::Scalar(pair) => {
                let checked = pair.rows();
                if checked.definitions.len() != self.rows {
                    return Err(stage
                        .source
                        .missing::<()>("aggregate scalar definition census")
                        .unwrap_err()
                        .into());
                }
                for (input, row) in checked.definitions.iter().enumerate() {
                    budget.charge_work(3)?;
                    if stage
                        .input
                        .definitions()
                        .get(input)
                        .is_none_or(|actual| actual.coordinate != row.input)
                    {
                        return Err(stage
                            .source
                            .missing::<()>("aggregate scalar definition census")
                            .unwrap_err()
                            .into());
                    }
                    let start = row.outputs.start as usize;
                    let end = start
                        .checked_add(row.outputs.len as usize)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    let descendants = checked.definition_outputs.get(start..end).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "aggregate scalar descendant range",
                        ),
                    )?;
                    for descendant in descendants {
                        let output = aggregate_definition_index_v30(
                            stage.output,
                            descendant.output,
                            budget,
                        )?;
                        budget.charge_work(self.columns)?;
                        for column in 0..self.columns {
                            next[output * self.columns + column] |=
                                self.relation[input * self.columns + column];
                        }
                    }
                }
            }
            AggregateSourceStageRelationV30::Aggregate(_) => {
                let index =
                    stage
                        .aggregate_index
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "aggregate occurrence index absent",
                        ))?;
                for (input, row) in stage.input.definitions().iter().enumerate() {
                    budget.charge_work(2)?;
                    let output = index.definition(input, budget).map_err(|error| {
                        E::Optimization(
                            fe2o3_kernel_opt::OwnedAggregateFixedpointErrorV18::Aggregate(
                                fe2o3_kernel_opt::OwnedAggregateSsaErrorV18::Check(error),
                            ),
                        )
                    })?;
                    // A removed private pointer has no descendant. A selected
                    // read keeps its original ValueId in the checked Select copy.
                    if let Some(output) = output {
                        let output = aggregate_definition_index_v30(stage.output, output, budget)?;
                        if stage.output.definitions()[output].ty != row.ty {
                            return Err(stage
                                .source
                                .missing::<()>("aggregate retained definition type")
                                .unwrap_err()
                                .into());
                        }
                        budget.charge_work(self.columns)?;
                        for column in 0..self.columns {
                            next[output * self.columns + column] |=
                                self.relation[input * self.columns + column];
                        }
                    }
                }
            }
        }
        let old_credit = self.relation.capacity();
        let columns = self.columns;
        drop(self);
        budget.release_storage(old_credit)?;
        Ok(Self {
            owner: std::ptr::from_ref(stage.output.owner()) as usize,
            next_stage: stage
                .ordinal
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?,
            rows,
            columns,
            relation: next,
        })
    }

    fn contains(
        &self,
        inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        stages: usize,
        column: usize,
        final_definition: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<bool> {
        budget.charge_work(4)?;
        if self.owner != std::ptr::from_ref(inventory.owner()) as usize
            || self.next_stage != stages
            || self.rows != inventory.definitions().len()
            || column >= self.columns
            || self.relation.len() != argument_product_v1(self.rows, self.columns)?
        {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "complete aggregate definition transport",
            ));
        }
        let row = aggregate_definition_index_v30(inventory, final_definition, budget)?;
        Ok(self.relation[row * self.columns + column] == 1)
    }
}
