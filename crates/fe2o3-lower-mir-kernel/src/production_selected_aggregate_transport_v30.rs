//! Selected source requests through every actual scalar/aggregate stage.
//! This private coordinate owner is not the final pointer-graph conjunction.
use super::*;

#[path = "production_selected_aggregate_source_v30.rs"]
mod consumer;

pub(super) struct SelectedAggregateTransportV30<'source> {
    initial: &'source ProductionOptimizedSourceCorrespondenceV18<'source>,
    definitions: Vec<Definition>,
    operations: Vec<AggregateOperationV30>,
    edges: Vec<AggregateEdgeV30>,
    current: Vec<SelectedTransportRowV30>,
    outputs: Vec<Definition>,
    coordinates: AggregateCoordinateTransportV30,
    endpoints: Vec<usize>,
    finalized: bool,
}

impl AggregateStageStateV30 for SelectedAggregateTransportV30<'_> {
    fn retained_storage(&self) -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            aggregate_vector_credit_v30(&self.definitions)?,
            aggregate_vector_credit_v30(&self.operations)?,
            aggregate_vector_credit_v30(&self.edges)?,
            aggregate_vector_credit_v30(&self.current)?,
            aggregate_vector_credit_v30(&self.outputs)?,
            aggregate_vector_credit_v30(&self.endpoints)?,
            self.coordinates.retained_storage()?,
        ])
    }
}

fn selected_aggregate_transport_headers_v30() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<Result<T, ProductionAggregateSourceErrorV30>>())?,
            size_of::<SourceOwnedResultV18<T>>(),
        ])
    }
    type Frame<'a> = (
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        &'a ProductionScopedAggregateMemoryV31<'a, 'a, 'a>,
        &'a AggregateSourceStageV30<'a>,
        &'a ProductionAggregateSourceOutputHandoffV30<'a, 'a>,
        [&'a Inventory<'a>; 2],
        &'a mut ArgumentBudgetV1<'a>,
        &'a AggregateDefinitionTransportV30,
        [usize; 24],
        [Option<usize>; 4],
        [Option<Definition>; 4],
        [SourceOwnedResultV18<()>; 4],
        [Result<(), ProductionAggregateSourceErrorV30>; 4],
        [SelectedTransportRowV30; 2],
        [&'a SelectedTransportRowV30; 2],
        &'a mut SelectedTransportRowV30,
        [Definition; 6],
        [EdgeArgument; 3],
        [Option<EdgeArgument>; 2],
        [EdgeControl; 2],
        [OutputUse; 2],
        [Option<OutputUse>; 2],
        UseCoordinate,
        CanonicalKirTransitionRangeV1,
        &'a fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirUseRefV1,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        std::mem::align_of::<Frame<'_>>(),
        h::<SelectedAggregateTransportV30<'_>>()?,
        h::<Option<SelectedAggregateTransportV30<'_>>>()?,
        h::<Vec<Definition>>()?,
        h::<Vec<AggregateOperationV30>>()?,
        h::<Vec<AggregateEdgeV30>>()?,
        h::<Vec<AggregateFunctionV30>>()?,
        h::<Vec<SelectedTransportRowV30>>()?,
        h::<Vec<CanonicalKirTransitionRangeV1>>()?,
        h::<Vec<usize>>()?,
        h::<Control<'_, '_, '_>>()?,
        h::<Option<Control<'_, '_, '_>>>()?,
        h::<fe2o3_kernel_analysis::CanonicalKirControlIndexStorageV1>()?,
        aggregate_definition_headers_v30()?,
        aggregate_coordinate_headers_v30()?,
    ])
}

fn sorted_unique<T: Copy + Ord>(
    rows: &mut Vec<T>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    call_splice_sort_work_v1(argument_product_v1(rows.len(), size_of::<T>())?, budget)
        .map_err(source_address_call_error_v29)
        .map_err(source_emission_error_v18)?;
    rows.sort_unstable();
    budget.charge_work(rows.len())?;
    rows.dedup();
    Ok(())
}

impl<'source> SelectedAggregateTransportV30<'source> {
    fn seed(
        initial: &'source ProductionOptimizedSourceCorrespondenceV18<'source>,
        memory: &ProductionScopedAggregateMemoryV31<'_, '_, '_>,
        stage: &AggregateSourceStageV30<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionAggregateSourceErrorV30> {
        stage.check(budget)?;
        check_memory_stage(memory, stage, budget)?;
        initial
            .original
            .global_expression_entry_v23(initial, budget)?;
        if stage.ordinal != 0
            || !std::ptr::eq(stage.source, initial.original.source)
            || !std::ptr::eq(stage.input.owner(), initial.checked.input().owner())
            || !std::ptr::eq(stage.output.owner(), initial.checked.output().owner())
        {
            return stage
                .source
                .missing("selected aggregate initial source owner differs")
                .map_err(Into::into);
        }
        let roots = stage.source.root_count(budget)?;
        if initial.index.selected_roots.len() != roots {
            return stage
                .source
                .missing("selected aggregate source root census differs")
                .map_err(Into::into);
        }
        let mut functions =
            source_reference_emission_vec_v29(roots, budget).map_err(source_argument_error_v18)?;
        let mut argument_count = 0;
        for root in 0..roots {
            initial.replay_selected_transport_v30(root, budget)?;
            let (_, physical) = stage.source.root(root, budget)?;
            budget.charge_work(3)?;
            let function = stage.input.functions().get(physical).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("selected aggregate root function"),
            )?;
            argument_count =
                argument_sum_v1(&[argument_count, function.function.signature.parameters.len()])?;
            functions.push(function.coordinate);
        }
        let mut count = [argument_count, 0, 0];
        for row in &initial.index.selected {
            budget.charge_work(3)?;
            match row {
                SelectedTransportRowV30::Definition { .. } => {
                    count[0] = argument_sum_v1(&[count[0], 1])?
                }
                SelectedTransportRowV30::Incoming { relation, .. }
                | SelectedTransportRowV30::Invocation { relation, .. } => {
                    count[0] = argument_sum_v1(&[count[0], 2])?;
                    count[2] =
                        argument_sum_v1(&[count[2], usize::from(relation.output.is_some())])?;
                }
                SelectedTransportRowV30::Access {
                    disposition: ProductionOptimizedSourceOperationV18::Retained { .. },
                    ..
                } => {
                    count[1] = argument_sum_v1(&[count[1], 1])?;
                }
                SelectedTransportRowV30::Guard { control, .. } => {
                    count[2] = argument_sum_v1(&[
                        count[2],
                        usize::from(matches!(control.placement, EdgePlacement::Retained(_))),
                    ])?;
                }
                _ => (),
            }
        }
        let mut definitions = source_reference_emission_vec_v29(count[0], budget)
            .map_err(source_argument_error_v18)?;
        let mut operations = source_reference_emission_vec_v29(count[1], budget)
            .map_err(source_argument_error_v18)?;
        let mut edges = source_reference_emission_vec_v29(count[2], budget)
            .map_err(source_argument_error_v18)?;
        for function in &functions {
            budget.charge_work(1)?;
            let row = &stage.input.functions()[function.0 as usize];
            for argument in 0..row.function.signature.parameters.len() {
                budget.charge_work(2)?;
                let at = argument_sum_v1(&[row.definitions.start, argument])?;
                let definition = stage.input.definitions().get(at).ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "selected aggregate original ABI argument",
                    ),
                )?;
                if at >= row.definitions.end
                    || !matches!(definition.coordinate, Definition::FunctionArgument { function: found, argument: ordinal } if found == *function && ordinal as usize == argument)
                {
                    return stage
                        .source
                        .missing("selected aggregate original ABI coordinate differs")
                        .map_err(Into::into);
                }
                definitions.push(definition.coordinate);
            }
        }
        for row in &initial.index.selected {
            budget.charge_work(3)?;
            match *row {
                SelectedTransportRowV30::Definition { relation, .. } => {
                    definitions.push(relation.input)
                }
                SelectedTransportRowV30::Incoming { relation, .. }
                | SelectedTransportRowV30::Invocation { relation, .. } => {
                    definitions.push(relation.incoming.input);
                    definitions.push(relation.target.input);
                    if relation.output.is_some() {
                        edges.push(relation.input.edge);
                    }
                }
                SelectedTransportRowV30::Access {
                    disposition: ProductionOptimizedSourceOperationV18::Retained { input, .. },
                    ..
                } => operations.push(input),
                SelectedTransportRowV30::Guard { control, input, .. } => {
                    if matches!(control.placement, EdgePlacement::Retained(_)) {
                        edges.push(input);
                    }
                }
                _ => (),
            }
        }
        if [definitions.len(), operations.len(), edges.len()] != count {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        sorted_unique(&mut definitions, budget)?;
        sorted_unique(&mut operations, budget)?;
        sorted_unique(&mut edges, budget)?;
        let coordinates = AggregateCoordinateTransportV30::seed(
            stage.input,
            &definitions,
            &operations,
            &edges,
            &functions,
            budget,
        )?;
        let credit = aggregate_vector_credit_v30(&functions)?;
        drop(functions);
        budget.release_storage(credit)?;
        let count = argument_sum_v1(&[argument_product_v1(stage.chain.rounds().len(), 2)?, 1])?;
        let mut endpoints =
            source_reference_emission_vec_v29(count, budget).map_err(source_argument_error_v18)?;
        budget.charge_work(1)?;
        endpoints.push(std::ptr::from_ref(stage.input.owner()) as usize);
        let mut current = source_reference_emission_vec_v29(initial.index.selected.len(), budget)
            .map_err(source_argument_error_v18)?;
        budget.charge_work(argument_product_v1(
            initial.index.selected.len(),
            size_of::<SelectedTransportRowV30>(),
        )?)?;
        current.extend_from_slice(&initial.index.selected);
        let outputs =
            source_reference_emission_vec_v29(0, budget).map_err(source_argument_error_v18)?;
        Ok(Self {
            initial,
            definitions,
            operations,
            edges,
            current,
            outputs,
            coordinates,
            endpoints,
            finalized: false,
        })
    }

    fn advance(
        mut self,
        memory: &ProductionScopedAggregateMemoryV31<'_, '_, '_>,
        stage: &AggregateSourceStageV30<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionAggregateSourceErrorV30> {
        stage.check(budget)?;
        check_memory_stage(memory, stage, budget)?;
        budget.charge_work(4)?;
        if self.finalized
            || self.endpoints.len() != argument_sum_v1(&[stage.ordinal, 1])?
            || self.endpoints.last().copied()
                != Some(std::ptr::from_ref(stage.input.owner()) as usize)
            || self.endpoints.len() == self.endpoints.capacity()
        {
            return stage
                .source
                .missing("selected aggregate stage order or owner differs")
                .map_err(Into::into);
        }
        self.coordinates = self.coordinates.advance(stage, budget)?;
        if stage.ordinal != 0 {
            self.advance_occurrences(stage, budget)?;
        }
        self.endpoints
            .push(std::ptr::from_ref(stage.output.owner()) as usize);
        Ok(self)
    }

    fn finish(
        &mut self,
        memory: &ProductionScopedAggregateMemoryV31<'_, '_, '_>,
        handoff: &ProductionAggregateSourceOutputHandoffV30<'_, '_>,
        output: &Inventory<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30> {
        handoff.owned.check(budget)?;
        if !std::ptr::eq(memory.chain(budget)?, handoff) {
            return handoff
                .owned
                .source
                .missing("selected aggregate memory handoff differs")
                .map_err(Into::into);
        }
        memory.check_final_owner(output.owner(), budget)?;
        let chain = handoff.output(budget)?;
        let stages = argument_product_v1(chain.rounds().len(), 2)?;
        budget.charge_work(8)?;
        if self.finalized
            || !std::ptr::eq(handoff.owned.source, self.initial.original.source)
            || !std::ptr::eq(output.owner(), chain.owner())
            || self.coordinates.definitions.owner != std::ptr::from_ref(output.owner()) as usize
            || self.coordinates.definitions.next_stage != stages
            || self.endpoints.len() != argument_sum_v1(&[stages, 1])?
            || self.endpoints.first().copied()
                != Some(std::ptr::from_ref(self.initial.checked.input().owner()) as usize)
        {
            return handoff
                .owned
                .source
                .missing("selected aggregate final owner or stage census differs")
                .map_err(Into::into);
        }
        for (round, row) in chain.rounds().iter().enumerate() {
            budget.charge_work(2)?;
            if self.endpoints[round * 2 + 1] != std::ptr::from_ref(row.scalar().owner()) as usize
                || self.endpoints[round * 2 + 2]
                    != std::ptr::from_ref(row.aggregate().output()) as usize
            {
                return handoff
                    .owned
                    .source
                    .missing("selected aggregate final endpoint order differs")
                    .map_err(Into::into);
            }
        }
        self.finish_definitions(output, budget)?;
        self.check_access_definitions(output, budget)?;
        self.finalized = true;
        Ok(())
    }
}

fn check_memory_stage(
    memory: &ProductionScopedAggregateMemoryV31<'_, '_, '_>,
    stage: &AggregateSourceStageV30<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let owner = memory.chain(budget)?;
    budget.charge_work(4)?;
    if !std::ptr::eq(owner.output(budget)?, stage.chain) {
        return stage
            .source
            .missing("selected aggregate memory stage chain differs");
    }
    let census = memory.memory(budget)?;
    if !census.endpoint_belongs_to(stage.ordinal, stage.input.owner())
        || !census.endpoint_belongs_to(argument_sum_v1(&[stage.ordinal, 1])?, stage.output.owner())
    {
        return stage
            .source
            .missing("selected aggregate memory stage endpoints differ");
    }
    Ok(())
}

fn find<T: Ord>(
    rows: &[T],
    key: &T,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    charge_execution_cfg_lookup_v29(rows.len(), budget).map_err(source_emission_error_v18)?;
    rows.binary_search(key).map_err(|_| {
        ProductionSourceOwnedViewErrorV18::Binding(
            "selected aggregate exact requested occurrence is absent",
        )
    })
}

fn descendant(
    definitions: &[Definition],
    coordinates: &AggregateCoordinateTransportV30,
    output: &Inventory<'_>,
    original: Definition,
    actual: Definition,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let column = find(definitions, &original, budget)?;
    if !coordinates.definitions.contains(
        output,
        coordinates.definitions.next_stage,
        column,
        actual,
        budget,
    )? {
        return resources::binding(
            "selected aggregate actual definition lost its exact original source",
        );
    }
    Ok(())
}

impl SelectedAggregateTransportV30<'_> {
    fn advance_occurrences(
        &mut self,
        stage: &AggregateSourceStageV30<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30> {
        let (control, credit) = match stage.relation {
            AggregateSourceStageRelationV30::Scalar(pair) => {
                let (control, receipt) =
                    Control::derive_v18(pair, budget).map_err(transition_error)?;
                budget.reserve_storage(receipt.retained_storage())?;
                (Some(control), receipt.retained_storage())
            }
            AggregateSourceStageRelationV30::Aggregate(_) => (None, 0),
        };
        for row in &mut self.current {
            budget.charge_work(4)?;
            match row {
                SelectedTransportRowV30::Access {
                    disposition,
                    output_pointer,
                    output_value,
                    ..
                } => {
                    let ProductionOptimizedSourceOperationV18::Retained { input, output } =
                        disposition
                    else {
                        continue;
                    };
                    let at = find(&self.operations, input, budget)?;
                    *output = *self
                        .coordinates
                        .operations
                        .get(at)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let operation =
                        source_operation_row_v18(stage.output, *output, budget)?.operation;
                    let access = source_address_value_access_v29(operation)
                        .map_err(source_emission_error_v18)?
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "selected aggregate actual memory access is absent",
                        ))?;
                    if access.object {
                        return stage
                            .source
                            .missing("selected aggregate external access changed family")
                            .map_err(Into::into);
                    }
                    *output_pointer = Some(definition(
                        stage.output,
                        output.block.function,
                        access.pointer,
                        budget,
                    )?);
                    *output_value = Some(definition(
                        stage.output,
                        output.block.function,
                        access.value,
                        budget,
                    )?);
                }
                SelectedTransportRowV30::Incoming { relation, .. }
                | SelectedTransportRowV30::Invocation { relation, .. } => {
                    let Some(previous) = relation.output else {
                        continue;
                    };
                    let mapped_edge = *self
                        .coordinates
                        .edges
                        .get(find(&self.edges, &relation.input.edge, budget)?)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let next = if let Some(control) = &control {
                        let next_control = control
                            .edge(previous.edge, budget)
                            .map_err(transition_error)?;
                        if next_control.placement != EdgePlacement::Retained(mapped_edge) {
                            return stage
                                .source
                                .missing("selected aggregate source cut changed exact successor")
                                .map_err(Into::into);
                        }
                        relation.control = EdgeControl {
                            placement: next_control.placement,
                            executable: relation.control.executable && next_control.executable,
                        };
                        control
                            .edge_argument(previous, budget)
                            .map_err(transition_error)?
                    } else {
                        let index = stage
                            .aggregate_index
                            .ok_or(ArgumentResourceV1::Accounting)?;
                        let incoming = argument(stage.input, previous, budget)?;
                        let target = index
                            .definition(incoming.target_definition, budget)
                            .map_err(aggregate_memory_check_error_v31)?
                            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                "selected aggregate pointer parameter was removed",
                            ))?;
                        let Definition::BlockArgument {
                            block,
                            argument: ordinal,
                        } = target
                        else {
                            return stage
                                .source
                                .missing(
                                    "selected aggregate pointer parameter changed representation",
                                )
                                .map_err(Into::into);
                        };
                        if edge(stage.output, mapped_edge, budget)?.target != block {
                            return stage
                                .source
                                .missing("selected aggregate pointer parameter changed target")
                                .map_err(Into::into);
                        }
                        relation.control.placement = EdgePlacement::Retained(mapped_edge);
                        Some(EdgeArgument {
                            edge: mapped_edge,
                            argument: ordinal,
                        })
                    };
                    relation.output = next;
                    let (incoming, target) = if let Some(next) = next {
                        if next.edge != mapped_edge {
                            return stage
                                .source
                                .missing("selected aggregate argument left its exact successor")
                                .map_err(Into::into);
                        }
                        let row = argument(stage.output, next, budget)?;
                        let incoming = stage
                            .output
                            .definitions()
                            .get(row.incoming_definition)
                            .ok_or(ArgumentResourceV1::Accounting)?
                            .coordinate;
                        let target = stage
                            .output
                            .definitions()
                            .get(row.target_definition)
                            .ok_or(ArgumentResourceV1::Accounting)?
                            .coordinate;
                        descendant(
                            &self.definitions,
                            &self.coordinates,
                            stage.output,
                            relation.incoming.input,
                            incoming,
                            budget,
                        )?;
                        descendant(
                            &self.definitions,
                            &self.coordinates,
                            stage.output,
                            relation.target.input,
                            target,
                            budget,
                        )?;
                        (Some(incoming), Some(target))
                    } else {
                        (None, None)
                    };
                    relation.output_incoming = incoming;
                    relation.output_target = target;
                }
                SelectedTransportRowV30::Guard {
                    control: prior,
                    input,
                    output_selector,
                    ..
                } => {
                    let EdgePlacement::Retained(previous) = prior.placement else {
                        continue;
                    };
                    let mapped_edge = *self
                        .coordinates
                        .edges
                        .get(find(&self.edges, input, budget)?)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let selector =
                        output_selector.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "selected aggregate retained guard lost its selector",
                        ))?;
                    if let Some(control) = &control {
                        let next = control.edge(previous, budget).map_err(transition_error)?;
                        if next.placement != EdgePlacement::Retained(mapped_edge) {
                            return stage
                                .source
                                .missing("selected aggregate guard changed exact successor")
                                .map_err(Into::into);
                        }
                        *prior = EdgeControl {
                            placement: next.placement,
                            executable: prior.executable && next.executable,
                        };
                        *output_selector = control
                            .operand(selector.coordinate, budget)
                            .map_err(transition_error)?;
                    } else {
                        let block = source_block_row_v18(stage.output, mapped_edge.source, budget)?;
                        let coordinate = UseCoordinate::TerminatorOperand {
                            block: mapped_edge.source,
                            operand: 0,
                        };
                        let actual = stage
                            .output
                            .uses()
                            .get(block.terminator_uses.start)
                            .filter(|row| {
                                block.terminator_uses.start < block.terminator_uses.end
                                    && row.coordinate == coordinate
                            })
                            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                "selected aggregate guard selector occurrence is absent",
                            ))?;
                        let definition = stage
                            .output
                            .definitions()
                            .get(actual.definition)
                            .ok_or(ArgumentResourceV1::Accounting)?
                            .coordinate;
                        prior.placement = EdgePlacement::Retained(mapped_edge);
                        *output_selector = Some(OutputUse {
                            coordinate,
                            definition,
                        });
                    }
                    if output_selector.is_none() {
                        return stage
                            .source
                            .missing("selected aggregate retained guard selector was erased")
                            .map_err(Into::into);
                    }
                }
                _ => (),
            }
        }
        drop(control);
        budget.release_storage(credit)?;
        Ok(())
    }

    fn finish_definitions(
        &mut self,
        output: &Inventory<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30> {
        let matrix = &self.coordinates.definitions;
        if matrix.columns != self.definitions.len()
            || matrix.rows != output.definitions().len()
            || matrix.relation.len() != argument_product_v1(matrix.rows, matrix.columns)?
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(matrix.relation.len())?;
        let count = matrix.relation.iter().filter(|&&bit| bit != 0).count();
        let mut outputs =
            source_reference_emission_vec_v29(count, budget).map_err(source_argument_error_v18)?;
        let mut ranges = source_reference_emission_vec_v29(matrix.columns, budget)
            .map_err(source_argument_error_v18)?;
        for column in 0..matrix.columns {
            budget.charge_work(2)?;
            let start = outputs.len();
            for row in 0..matrix.rows {
                budget.charge_work(2)?;
                if matrix.relation[row * matrix.columns + column] == 1 {
                    outputs.push(output.definitions()[row].coordinate);
                }
            }
            ranges.push(CanonicalKirTransitionRangeV1 {
                start: u32::try_from(start).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                len: u32::try_from(outputs.len() - start)
                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
            });
        }
        if outputs.len() != count {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let definitions = &self.definitions;
        let update = |relation: &mut SelectedDefinitionV30,
                      budget: &mut ArgumentBudgetV1<'_>|
         -> SourceOwnedResultV18<()> {
            relation.outputs = *ranges
                .get(find(definitions, &relation.input, budget)?)
                .ok_or(ArgumentResourceV1::Accounting)?;
            Ok(())
        };
        for row in &mut self.current {
            budget.charge_work(1)?;
            match row {
                SelectedTransportRowV30::Definition { relation, .. } => update(relation, budget)?,
                SelectedTransportRowV30::Incoming { relation, .. }
                | SelectedTransportRowV30::Invocation { relation, .. } => {
                    update(&mut relation.incoming, budget)?;
                    update(&mut relation.target, budget)?;
                }
                _ => (),
            }
        }
        let credit = argument_sum_v1(&[
            aggregate_vector_credit_v30(&ranges)?,
            aggregate_vector_credit_v30(&self.outputs)?,
        ])?;
        drop(ranges);
        let old = std::mem::replace(&mut self.outputs, outputs);
        drop(old);
        budget.release_storage(credit)?;
        Ok(())
    }

    fn check_access_definitions(
        &self,
        output: &Inventory<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        for (at, row) in self.current.iter().enumerate() {
            budget.charge_work(3)?;
            let SelectedTransportRowV30::Access {
                disposition,
                output_pointer,
                output_value,
                ..
            } = *row
            else {
                continue;
            };
            let Some(SelectedTransportRowV30::Definition {
                role: SelectedDefinitionRoleV30::AccessPointer,
                relation: pointer,
            }) = self.current.get(argument_sum_v1(&[at, 1])?)
            else {
                return resources::binding("selected aggregate access pointer source role differs");
            };
            let Some(SelectedTransportRowV30::Definition {
                role: SelectedDefinitionRoleV30::AccessValue,
                relation: value,
            }) = self.current.get(argument_sum_v1(&[at, 2])?)
            else {
                return resources::binding("selected aggregate access value source role differs");
            };
            match disposition {
                ProductionOptimizedSourceOperationV18::Retained { .. } => {
                    let actual_pointer =
                        output_pointer.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "selected aggregate access pointer is absent",
                        ))?;
                    let actual_value =
                        output_value.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "selected aggregate access value is absent",
                        ))?;
                    descendant(
                        &self.definitions,
                        &self.coordinates,
                        output,
                        pointer.input,
                        actual_pointer,
                        budget,
                    )?;
                    descendant(
                        &self.definitions,
                        &self.coordinates,
                        output,
                        value.input,
                        actual_value,
                        budget,
                    )?;
                }
                _ if output_pointer.is_some() || output_value.is_some() => {
                    return resources::binding(
                        "selected aggregate retired source access regained an endpoint",
                    );
                }
                _ => (),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_aggregate_lookup_has_independent_sixty_four_work_boundary() {
        for limit in [64, 63] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, 17);
            budget.reserve_storage(17).unwrap();
            let found = find(&[1u32, 3, 5, 7, 9], &1, &mut budget);
            if limit == 64 {
                assert_eq!(found.unwrap(), 0);
                assert_eq!(budget.work(), (2 + 2) * 16);
            } else {
                assert!(matches!(
                    found,
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Work(_)
                    ))
                ));
            }
            assert_eq!(budget.storage(), 17);
        }
    }

    #[test]
    fn selected_aggregate_sort_dedup_has_independent_paid_boundary() {
        for limit in [243, 242] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, 17);
            budget.reserve_storage(17).unwrap();
            let mut rows = vec![2u32, 1, 2];
            let result = sorted_unique(&mut rows, &mut budget);
            // Three four-byte keys, five paid binary-sort levels, four work
            // per level, followed by three distinctness visits.
            if limit == 243 {
                result.unwrap();
                assert_eq!(rows, [1, 2]);
                assert_eq!(budget.work(), 12 * 5 * 4 + 3);
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Work(_)
                    ))
                ));
            }
            assert_eq!(budget.storage(), 17);
        }
    }
}
