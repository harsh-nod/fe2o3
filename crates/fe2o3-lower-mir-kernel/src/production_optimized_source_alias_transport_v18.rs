use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as AliasBlockV18,
    CanonicalKirDefinitionCoordinateV1 as AliasDefinitionV18,
    CanonicalKirEdgeArgumentCoordinateV1 as AliasEdgeArgumentV18,
    CanonicalKirEdgeCoordinateV1 as AliasEdgeV18,
    CanonicalKirUseCoordinateV1 as AliasUseV18,
};

struct OptimizedSourceAliasTransportV18 {
    aliases: Vec<SourceAddressLogicalAliasV29>,
    uses: Vec<SourceAddressLogicalUseV29>,
    boundaries: Vec<SourceAddressBoundaryEventV29>,
}

struct OptimizedSourceCurrentnessEquationsV18<'view, 'kir> {
    function: &'view Function,
    graph: &'view SourceAddressMemoryV29<'kir>,
    slots: &'view [ScopedSourceSlotV29],
    accesses: &'view [SourceAddressAccessV29],
    kills: &'view [SourceAddressKillV29],
    initial: &'view [bool],
    lifetimes: &'view [SourceAddressLifetimeV29],
    births: &'view [SourceAddressBirthV29],
    failures: &'view [SourceIndexFailureV29],
    geometry: SourceAddressGeometryV29,
}

impl OptimizedSourceCurrentnessEquationsV18<'_, '_> {
    fn check(&self, transport: SourceAddressAliasTransportV29<'_>, budget: &mut ArgumentBudgetV1<'_>)
        -> Result<(), ProductionSemanticKirErrorV1>
    {
        check_source_address_currentness_transport_v29(self.function, self.graph, self.slots,
            self.accesses, self.kills, self.initial, self.lifetimes, self.births, self.failures,
            self.geometry, transport, budget)
    }
}

impl OptimizedSourceAliasTransportV18 {
    fn equations(&self) -> SourceAddressAliasTransportV29<'_> {
        SourceAddressAliasTransportV29 {
            aliases: &self.aliases, uses: &self.uses, boundaries: Some(&self.boundaries),
        }
    }
}

#[derive(Clone, Copy)]
struct OptimizedAliasSegmentV18 {
    output: AliasBlockV18,
    ordinal: usize,
    incoming: Option<AliasEdgeV18>,
}

#[derive(Clone, Copy)]
enum OptimizedAliasUseOriginV18 {
    Operand(AliasUseV18),
    EdgeArgument(AliasEdgeArgumentV18),
}

#[derive(Clone, Copy)]
struct OptimizedAliasUseJoinV18 {
    input: OptimizedAliasUseOriginV18,
    output: SourceAddressLogicalUseV29,
}

#[derive(Clone, Copy)]
struct OptimizedAliasBoundaryV18 {
    actual: SourceAddressBoundaryEventV29,
    // Actual merge segment, original canonical gap, before/after boundary,
    // then the original within-gap occurrence. This is not source block order.
    order: [usize; 4],
}

#[derive(Clone, Copy)]
enum OptimizedAliasRecipeSourceV18 {
    Operation(fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1),
    MergedArgument { block: AliasBlockV18, incoming: AliasEdgeArgumentV18 },
}

fn optimized_alias_unique_descendant_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input: AliasDefinitionV18,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<(AliasDefinitionV18, ValueId)>> {
    let output = optimized.output_inventory(budget)?;
    let mut found = None;
    for descendant in optimized.definition_descendants(input, budget)? {
        budget.charge_work(3)?;
        let owner = match descendant.output {
            AliasDefinitionV18::FunctionArgument { function, .. } => function,
            AliasDefinitionV18::BlockArgument { block, .. } => block.function,
            AliasDefinitionV18::Result { operation, .. } => operation.block.function,
        };
        let row = optimized_source_definition_row_v18(output, descendant.output, budget)?;
        if owner != function || !matches!(row.ty, Type::Pointer(_)) {
            return original.source.missing("logical alias changed pointer descendant type or function");
        }
        let value = row.value.ok_or(ProductionSourceOwnedViewErrorV18::Binding("logical alias descendant has no value"))?;
        if found.replace((descendant.output, value)).is_some() {
            return original.source.missing("logical alias needs one exact actual pointer descendant");
        }
    }
    Ok(found)
}

fn optimized_alias_recipe_input_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    definitions: std::ops::Range<usize>,
    index: usize,
    aliases: &[Option<usize>],
    physical: &[Option<ValueId>],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<SourceAddressAliasInputV29> {
    budget.charge_work(2)?;
    if !definitions.contains(&index) {
        return original.source.missing("logical alias recipe changed original function");
    }
    let at = index - definitions.start;
    if let Some(alias) = aliases[at] { return Ok(SourceAddressAliasInputV29::Logical(alias)); }
    let value = physical[at].ok_or(ProductionSourceOwnedViewErrorV18::Binding(
        "logical alias recipe lacks an exact actual pointer definition"))?;
    Ok(SourceAddressAliasInputV29::Physical(value))
}

impl OptimizedAliasBoundaryV18 {
    fn key(&self) -> [usize; 6] {
        [self.actual.block.0 as usize, self.actual.gap,
            self.order[0], self.order[1], self.order[2], self.order[3]]
    }
}

fn optimized_alias_transport_headers_v18() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<OptimizedSourceAliasTransportV18>(),
        size_of::<OptimizedSourceCurrentnessEquationsV18<'_, '_>>(),
        // Segments, output uses, birth flags, recipes, logical/physical maps,
        // and the source-ordered boundary scratch are independently live.
        argument_product_v1(7, size_of::<Vec<usize>>())?,
        size_of::<OptimizedAliasCfgMeterV18<'_, '_, '_, '_>>(),
        size_of::<(Vec<Option<OptimizedAliasSegmentV18>>, Vec<OptimizedAliasUseJoinV18>)>(),
        size_of::<SourceOwnedResultV18<(Vec<Option<OptimizedAliasSegmentV18>>, Vec<OptimizedAliasUseJoinV18>)>>(),
        size_of::<SourceOwnedResultV18<OptimizedSourceAliasTransportV18>>(),
        size_of::<SourceOwnedResultV18<Option<(AliasDefinitionV18, ValueId)>>>(),
        size_of::<SourceOwnedResultV18<(AliasBlockV18, usize, Option<(BlockId, usize)>)>>(),
        size_of::<OptimizedAliasBoundaryV18>(),
        size_of::<Option<[usize; 6]>>(),
        size_of::<[usize; 3]>(),
        size_of::<Result<usize, usize>>(),
    ])
}

// Sorting inert rows is not source authorization. The producer authenticates
// every row and its key before calling this helper; the equation engine checks
// complete event coverage again before using the returned order.
fn optimized_alias_order_boundaries_v18(
    ordered: &mut [OptimizedAliasBoundaryV18],
    lifetimes: &mut [SourceAddressLifetimeV29],
    kills: &[SourceAddressKillV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<SourceAddressBoundaryEventV29>> {
    let scratch_header = size_of::<Vec<u8>>() + size_of::<Option<(BlockId, usize)>>();
    budget.reserve_storage(scratch_header)?;
    let mut seen_kills = emission_vec_v1(kills.len(), budget).map_err(immutable_memory_error_v29)?;
    budget.charge_work(kills.len())?;
    seen_kills.resize(kills.len(), 0_u8);
    for pair in kills.windows(2) {
        budget.charge_work(3)?;
        if (pair[0].block, pair[0].gap, pair[0].slot) >= (pair[1].block, pair[1].gap, pair[1].slot) {
            return Err(ProductionSourceOwnedViewErrorV18::Binding("logical alias physical kills are not canonical"));
        }
    }
    private_array_heapsort_v1(ordered, OptimizedAliasBoundaryV18::key,
        &mut SourceCorrespondenceWorkV18(budget), || ArgumentResourceV1::Arithmetic.into())?;
    let mut boundaries = emission_vec_v1(ordered.len(), budget).map_err(immutable_memory_error_v29)?;
    let (mut next_lifetime, mut next_kill) = (0, 0);
    let mut previous = None;
    let mut point = None;
    for mut event in ordered.iter().copied() {
        budget.charge_work(3)?;
        if previous.is_some_and(|key| key >= event.key()) {
            return Err(ProductionSourceOwnedViewErrorV18::Binding("logical alias source boundary order is not unique"));
        }
        previous = Some(event.key());
        let actual_point = (event.actual.block, event.actual.gap);
        if point != Some(actual_point) {
            if let Some(previous_point) = point {
                optimized_alias_append_gap_kills_v18(previous_point, kills, &seen_kills,
                    &mut next_kill, &mut boundaries, budget)?;
            }
            point = Some(actual_point);
        }
        match event.actual.kind {
            SourceAddressBoundaryKindV29::Lifetime(index) => {
                let row = lifetimes.get_mut(index).ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "logical alias source lifetime index"))?;
                if (row.block, row.gap) != (event.actual.block, event.actual.gap) {
                    return Err(ProductionSourceOwnedViewErrorV18::Binding("logical alias lifetime changed output position"));
                }
                // Renumber only after proving the actual segment and original
                // gap/sequence. This preserves source order across merged blocks.
                row.sequence = next_lifetime;
                event.actual.kind = SourceAddressBoundaryKindV29::Lifetime(next_lifetime);
                next_lifetime += 1;
            }
            SourceAddressBoundaryKindV29::Kill(index) => {
                let row = kills.get(index).ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "logical alias source kill index"))?;
                if (row.block, row.gap) != (event.actual.block, event.actual.gap) {
                    return Err(ProductionSourceOwnedViewErrorV18::Binding("logical alias kill changed output position"));
                }
                // Every source occurrence must join its exact physical key.
                // Repeated clears at one gap are idempotent, not extra events.
                seen_kills[index] = 1;
                continue;
            }
            SourceAddressBoundaryKindV29::Alias(_) => {}
        }
        boundaries.push(event.actual);
    }
    if let Some(point) = point {
        optimized_alias_append_gap_kills_v18(point, kills, &seen_kills,
            &mut next_kill, &mut boundaries, budget)?;
    }
    if next_lifetime != lifetimes.len() || next_kill != kills.len() {
        return Err(ProductionSourceOwnedViewErrorV18::Binding("logical alias source transition coverage changed"));
    }
    private_array_heapsort_v1(lifetimes, |row| [row.block.0 as usize, row.gap, row.sequence],
        &mut SourceCorrespondenceWorkV18(budget), || ArgumentResourceV1::Arithmetic.into())?;
    let scratch = argument_sum_v1(&[scratch_header, seen_kills.capacity()])?;
    drop(seen_kills);
    budget.release_storage(scratch)?;
    Ok(boundaries)
}

fn optimized_alias_append_gap_kills_v18(
    point: (BlockId, usize),
    kills: &[SourceAddressKillV29],
    seen: &[u8],
    next: &mut usize,
    boundaries: &mut Vec<SourceAddressBoundaryEventV29>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    // The closed virtual recipes read registers/object liveness, never holder
    // cells. A Kill only clears a holder and its origin; lifetime events can
    // also clear cells but cannot set them. Thus clears commute with these
    // virtual events at one actual gap, never across an actual operation.
    while let Some(row) = kills.get(*next)
        && (row.block, row.gap) == point
    {
        budget.charge_work(3)?;
        if seen.get(*next) != Some(&1) || boundaries.len() == boundaries.capacity() {
            return Err(ProductionSourceOwnedViewErrorV18::Binding("logical alias source transition coverage changed"));
        }
        boundaries.push(SourceAddressBoundaryEventV29 {
            block: row.block, gap: row.gap, kind: SourceAddressBoundaryKindV29::Kill(*next),
        });
        *next += 1;
    }
    Ok(())
}

struct OptimizedAliasCfgMeterV18<'a, 'g, 'b, 'w> {
    source: &'a ProductionSourceOwnedViewV18<'g>,
    budget: &'b mut ArgumentBudgetV1<'w>,
}

impl fe2o3_mir_model::SemanticAssertionMeterV1 for OptimizedAliasCfgMeterV18<'_, '_, '_, '_> {
    type Error = ProductionSourceOwnedViewErrorV18;
    fn charge_work(&mut self, work: usize) -> SourceOwnedResultV18<()> {
        self.source.check_query_v18(self.budget)?;
        self.budget.charge_work(work).map_err(|error| self.source.retain_query_resource_error_v18(error))
    }
    fn reserve_storage(&mut self, bytes: usize) -> SourceOwnedResultV18<()> {
        self.source.check_query_v18(self.budget)?;
        self.budget.reserve_storage(bytes).map_err(|error| self.source.retain_query_resource_error_v18(error))
    }
}

fn optimized_alias_use_definition_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    origin: OptimizedAliasUseOriginV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    budget.charge_work(3)?;
    match origin {
        OptimizedAliasUseOriginV18::Operand(coordinate) => {
            let (range, ordinal) = match coordinate {
                AliasUseV18::OperationOperand { operation, operand } =>
                    (optimized_source_operation_row_v18(original.inventory, operation, budget)?.operands.clone(), operand),
                AliasUseV18::TerminatorOperand { block, operand } =>
                    (optimized_source_block_row_v18(original.inventory, block, budget)?.terminator_uses.clone(), operand),
            };
            if ordinal as usize >= range.len() {
                return original.source.missing("logical alias original operand ordinal");
            }
            let row = original.inventory.uses().get(range.start + ordinal as usize)
                .filter(|row| row.coordinate == coordinate)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("logical alias original operand"))?;
            Ok(row.definition)
        }
        OptimizedAliasUseOriginV18::EdgeArgument(coordinate) => {
            let block = optimized_source_block_row_v18(original.inventory, coordinate.edge.source, budget)?;
            if coordinate.edge.successor as usize >= block.edges.len() {
                return original.source.missing("logical alias original successor ordinal");
            }
            let edge = original.inventory.edges().get(block.edges.start + coordinate.edge.successor as usize)
                .filter(|row| row.coordinate == coordinate.edge)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("logical alias original edge"))?;
            if coordinate.argument as usize >= edge.bindings.len() {
                return original.source.missing("logical alias original edge payload ordinal");
            }
            let row = original.inventory.edge_arguments().get(edge.bindings.start + coordinate.argument as usize)
                .filter(|row| row.coordinate == coordinate)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("logical alias original edge payload"))?;
            Ok(row.incoming_definition)
        }
    }
}

// This is a borrowed checked-output census. It grants no alias recipe or birth
// authority; original definitions and source births are joined separately.
fn optimized_alias_output_census_v18(
    original: &CheckedSourceMemoryV29<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(Vec<Option<OptimizedAliasSegmentV18>>, Vec<OptimizedAliasUseJoinV18>)> {
    use ProductionOptimizedSourceCfgEventV18 as Event;
    let relation = original.correspondence;
    original.check(budget)?;
    optimized_source_endpoints_v18(relation, optimized, budget)?;
    let function = &relation.inventory.functions()[relation.source.root_row(original.root)?.function_ordinal];
    budget.reserve_storage(std::mem::size_of::<ProductionOptimizedSourceCfgRootV18<'_, '_>>())?;
    let cfg = optimized.output_root_cfg_v18(original.root, budget)?;
    let mut segments = emission_vec_v1(function.blocks.len(), budget).map_err(immutable_memory_error_v29)?;
    budget.charge_work(function.blocks.len())?;
    segments.resize(function.blocks.len(), None);
    let capacity = argument_sum_v1(&[cfg.function().uses.len(), cfg.function().edge_arguments.len()])?;
    let mut uses = emission_vec_v1(capacity, budget).map_err(immutable_memory_error_v29)?;
    cfg.visit(&mut OptimizedAliasCfgMeterV18 { source: relation.source, budget }, |event, meter| {
        match event {
            Event::Block { actual, segments: chain } => {
                let mut incoming = None;
                for (ordinal, row) in chain.iter().enumerate() {
                    meter.charge_work(3)?;
                    if row.input.function != function.coordinate {
                        return Err(ProductionSourceOwnedViewErrorV18::Binding("logical alias foreign merge segment"));
                    }
                    let cell = segments.get_mut(row.input.block as usize)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding("logical alias merge segment ordinal"))?;
                    if cell.replace(OptimizedAliasSegmentV18 { output: actual.coordinate, ordinal, incoming }).is_some() {
                        return Err(ProductionSourceOwnedViewErrorV18::Binding("logical alias repeated merge segment"));
                    }
                    incoming = row.connector;
                }
            }
            Event::Operation { actual, operands, operand_origins, .. } => {
                for (operand, origin) in operands.iter().zip(operand_origins) {
                    meter.charge_work(3)?;
                    let AliasUseV18::OperationOperand { operation, operand: ordinal } = operand.coordinate else {
                        return Err(ProductionSourceOwnedViewErrorV18::Binding("logical alias output operation role"));
                    };
                    if origin.output != operand.coordinate || operation != actual.coordinate || uses.len() >= capacity {
                        return Err(ProductionSourceOwnedViewErrorV18::Binding("logical alias output operand lineage"));
                    }
                    let block = cfg.inventory().blocks()[cfg.function().blocks.start + operation.block.block as usize].block.id;
                    uses.push(OptimizedAliasUseJoinV18 {
                        input: OptimizedAliasUseOriginV18::Operand(origin.input),
                        output: SourceAddressLogicalUseV29 { block, operation: Some(operation.operation as usize),
                            successor: None, operand: ordinal as usize, value: operand.value, alias: usize::MAX },
                    });
                }
            }
            Event::Terminator { actual, operands, operand_origins, arguments, argument_origins, .. } => {
                if matches!(actual.terminator, Terminator::Return { .. }) {
                    for (operand, origin) in operands.iter().zip(operand_origins) {
                        meter.charge_work(3)?;
                        let AliasUseV18::TerminatorOperand { block, operand: ordinal } = operand.coordinate else {
                            return Err(ProductionSourceOwnedViewErrorV18::Binding("logical alias output return role"));
                        };
                        if block != actual.coordinate || origin.output != operand.coordinate || uses.len() >= capacity {
                            return Err(ProductionSourceOwnedViewErrorV18::Binding("logical alias output return lineage"));
                        }
                        uses.push(OptimizedAliasUseJoinV18 {
                            input: OptimizedAliasUseOriginV18::Operand(origin.input),
                            output: SourceAddressLogicalUseV29 { block: actual.block.id, operation: None,
                                successor: None, operand: ordinal as usize, value: operand.value, alias: usize::MAX },
                        });
                    }
                }
                for (argument, origin) in arguments.iter().zip(argument_origins) {
                    meter.charge_work(3)?;
                    if origin.output != argument.coordinate || argument.coordinate.edge.source != actual.coordinate
                        || uses.len() >= capacity
                    { return Err(ProductionSourceOwnedViewErrorV18::Binding("logical alias output edge lineage")); }
                    uses.push(OptimizedAliasUseJoinV18 {
                        input: OptimizedAliasUseOriginV18::EdgeArgument(origin.input),
                        output: SourceAddressLogicalUseV29 { block: actual.block.id, operation: None,
                            successor: Some(argument.coordinate.edge.successor as usize),
                            operand: argument.coordinate.argument as usize, value: argument.value, alias: usize::MAX },
                    });
                }
            }
        }
        Ok(())
    })?;
    optimized_source_endpoints_v18(relation, optimized, budget)?;
    Ok((segments, uses))
}

fn optimized_source_alias_transport_v18(
    original: &CheckedSourceMemoryV29<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    graph: &SourceAddressMemoryV29<'_>,
    slots: &[ScopedSourceSlotV29],
    prefix: &OptimizedMemoryGapPrefixV18,
    lifetimes: &mut [SourceAddressLifetimeV29],
    kills: &[SourceAddressKillV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<OptimizedSourceAliasTransportV18> {
    let relation = original.correspondence;
    original.check(budget)?;
    optimized_source_endpoints_v18(relation, optimized, budget)?;
    let input = &relation.inventory.functions()[relation.source.root_row(original.root)?.function_ordinal];
    let output = optimized_source_root_function_v18(relation, optimized, original.root, budget)?;
    let floor = budget.storage();
    budget.reserve_storage(optimized_alias_transport_headers_v18()?)?;
    let (segments, output_uses) = optimized_alias_output_census_v18(original, optimized, budget)?;
    let mut original_births = emission_vec_v1(input.operations.len(), budget).map_err(immutable_memory_error_v29)?;
    budget.charge_work(input.operations.len())?;
    original_births.resize(input.operations.len(), false);
    for birth in &original.pending.births {
        let coordinate = optimized_source_input_operation_v18(relation, original.root, birth.block, birth.operation, budget)?;
        let row = optimized_source_operation_row_v18(relation.inventory, coordinate, budget)?;
        budget.charge_work(4)?;
        if coordinate.block.function != input.coordinate
            || !matches!(row.operation.results.as_slice(), [result] if result.id == birth.result)
        { return relation.source.missing("logical alias source birth changed its exact result"); }
        let block = optimized_source_block_row_v18(relation.inventory, coordinate.block, budget)?;
        let index = block.operations.start + coordinate.operation as usize;
        let index = index.checked_sub(input.operations.start)
            .filter(|index| *index < original_births.len())
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("logical alias source birth ordinal"))?;
        if std::mem::replace(&mut original_births[index], true) {
            return relation.source.missing("logical alias source birth repeated an operation");
        }
    }
    let mut aliases = emission_vec_v1(input.definitions.len(), budget).map_err(immutable_memory_error_v29)?;
    let mut recipes = emission_vec_v1(input.definitions.len(), budget).map_err(immutable_memory_error_v29)?;
    let mut logical = emission_vec_v1(input.definitions.len(), budget).map_err(immutable_memory_error_v29)?;
    let mut physical = emission_vec_v1(input.definitions.len(), budget).map_err(immutable_memory_error_v29)?;
    budget.charge_work(argument_product_v1(2, input.definitions.len())?)?;
    logical.resize(input.definitions.len(), None);
    physical.resize(input.definitions.len(), None);
    for (ordinal, definition) in relation.inventory.definitions()[input.definitions.clone()].iter().enumerate() {
        budget.charge_work(1)?;
        if !matches!(definition.ty, Type::Pointer(_)) { continue; }
        let Some((actual, value)) = optimized_alias_unique_descendant_v18(
            relation, optimized, definition.coordinate, output.coordinate, budget)? else { continue; };
        physical[ordinal] = Some(value);
        let Some(slot) = graph.exact(value, budget).map_err(immutable_memory_error_v29)? else { continue; };
        let recipe = match definition.coordinate {
            AliasDefinitionV18::FunctionArgument { .. } => continue,
            AliasDefinitionV18::Result { operation, result: 0 } => match optimized.operation(operation, budget)? {
                ProductionOptimizedSourceOperationV18::Retained { .. } => continue,
                ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } =>
                    return relation.source.missing("unreachable alias acquired an actual definition"),
                ProductionOptimizedSourceOperationV18::Rewritten { .. } => OptimizedAliasRecipeSourceV18::Operation(operation),
            },
            AliasDefinitionV18::Result { .. } => return relation.source.missing("logical alias result arity remains pending"),
            AliasDefinitionV18::BlockArgument { block, argument } => {
                let segment = segments.get(block.block as usize).copied().flatten()
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding("logical alias original parameter segment"))?;
                if let Some(edge) = segment.incoming {
                    OptimizedAliasRecipeSourceV18::MergedArgument {
                        block, incoming: AliasEdgeArgumentV18 { edge, argument },
                    }
                } else if matches!(actual, AliasDefinitionV18::BlockArgument { block: actual, argument: actual_argument }
                    if actual == segment.output && actual_argument == argument)
                { continue; }
                else { return relation.source.missing("unmerged pointer parameter lacks exact currentness transport"); }
            }
        };
        logical[ordinal] = Some(aliases.len());
        aliases.push(SourceAddressLogicalAliasV29 { pointer: value, slot,
            recipe: SourceAddressAliasRecipeV29::Inherit(SourceAddressAliasInputV29::Physical(value)) });
        recipes.push(recipe);
    }
    let capacity = argument_sum_v1(&[aliases.len(), lifetimes.len(), original.pending.kills.len()])?;
    let mut ordered = emission_vec_v1(capacity, budget).map_err(immutable_memory_error_v29)?;
    for (index, recipe) in recipes.iter().enumerate() {
        budget.charge_work(1)?;
        let (block, gap, recipe) = match *recipe {
            OptimizedAliasRecipeSourceV18::MergedArgument { block, incoming } => {
                let definition = optimized_alias_use_definition_v18(relation,
                    OptimizedAliasUseOriginV18::EdgeArgument(incoming), budget)?;
                let input = optimized_alias_recipe_input_v18(relation, input.definitions.clone(),
                    definition, &logical, &physical, budget)?;
                (block, 0, SourceAddressAliasRecipeV29::Inherit(input))
            }
            OptimizedAliasRecipeSourceV18::Operation(coordinate) => {
                let row = optimized_source_operation_row_v18(relation.inventory, coordinate, budget)?;
                let block = optimized_source_block_row_v18(relation.inventory, coordinate.block, budget)?;
                let original_ordinal = block.operations.start + coordinate.operation as usize - input.operations.start;
                let operand = |ordinal, budget: &mut ArgumentBudgetV1<'_>| -> SourceOwnedResultV18<_> {
                    let definition = optimized_alias_use_definition_v18(relation,
                        OptimizedAliasUseOriginV18::Operand(AliasUseV18::OperationOperand {
                            operation: coordinate, operand: ordinal }), budget)?;
                    optimized_alias_recipe_input_v18(relation, input.definitions.clone(), definition,
                        &logical, &physical, budget)
                };
                let recipe = if original_births[original_ordinal] {
                    let OperationKind::Select { true_value, false_value, .. } = row.operation.kind else {
                        return relation.source.missing("erased non-Select source birth transport remains pending");
                    };
                    if true_value != false_value || aliases[index].pointer != slots[aliases[index].slot].origin.pointer
                        || !optimized_source_value_descends_v18(relation, optimized, input.coordinate, true_value,
                            output.coordinate, aliases[index].pointer, budget)?
                    { return relation.source.missing("erased source birth changed its checked backing descendant"); }
                    SourceAddressAliasRecipeV29::Birth
                } else {
                    match row.operation.kind {
                        OperationKind::Select { .. } => SourceAddressAliasRecipeV29::Select(
                            operand(1, budget)?, operand(2, budget)?),
                        OperationKind::GetElementPointer { .. }
                        | OperationKind::Cast { kind: CastKind::PointerToGeneric | CastKind::RestrictPointerAccess, .. } =>
                            SourceAddressAliasRecipeV29::Inherit(operand(0, budget)?),
                        _ => return relation.source.missing("rewritten pointer alias recipe remains pending"),
                    }
                };
                (coordinate.block, argument_sum_v1(&[coordinate.operation as usize, 1])?, recipe)
            }
        };
        aliases[index].recipe = recipe;
        let segment = segments.get(block.block as usize).copied().flatten()
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("logical alias definition segment"))?;
        let (actual_block, actual_gap) = prefix.canonical_boundary(relation, optimized, block, gap, budget)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("reachable alias definition has no output gap"))?;
        ordered.push(OptimizedAliasBoundaryV18 {
            actual: SourceAddressBoundaryEventV29 { block: actual_block, gap: actual_gap,
                kind: SourceAddressBoundaryKindV29::Alias(index) },
            order: [segment.ordinal, gap, 0, index],
        });
    }
    let owner = relation.source.root_row(original.root)?;
    let source_point = |block, gap, budget: &mut ArgumentBudgetV1<'_>| -> SourceOwnedResultV18<_> {
        let block_row = relation.inventory.block_for_id(input.coordinate, block, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("logical alias original boundary block"))?;
        let gap = immutable_memory_gap_v29(owner, block, gap, true, budget)?;
        let actual = prefix.canonical_boundary(relation, optimized, block_row.coordinate, gap, budget)?;
        Ok((block_row.coordinate, gap, actual))
    };
    for (source_ordinal, row) in original.pending.lifetimes.iter().enumerate() {
        budget.charge_work(2)?;
        let (input_block, input_gap, actual) = source_point(row.block, row.gap, budget)?;
        let Some((block, gap)) = actual else { continue; };
        let segment = segments.get(input_block.block as usize).copied().flatten()
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("logical alias lifetime segment"))?;
        // Before this producer, sequence is an exact original-row locator, not
        // permission to replay old block order on the merged output graph.
        let index = private_array_partition_v1(lifetimes,
            |row| [row.block.0 as usize, row.gap, row.sequence],
            [block.0 as usize, gap, source_ordinal], false,
            &mut SourceCorrespondenceWorkV18(budget))?;
        let mapped = lifetimes.get(index).ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "logical alias lifetime missing mapped original row"))?;
        budget.charge_work(5)?;
        if mapped.block != block || mapped.gap != gap || mapped.sequence != source_ordinal
            || mapped.slot != row.slot || mapped.live != row.live
        { return relation.source.missing("logical alias lifetime changed exact source transition"); }
        if ordered.len() >= capacity { return relation.source.missing("logical alias extra lifetime event"); }
        ordered.push(OptimizedAliasBoundaryV18 {
            actual: SourceAddressBoundaryEventV29 { block, gap, kind: SourceAddressBoundaryKindV29::Lifetime(index) },
            order: [segment.ordinal, input_gap, 1, row.sequence],
        });
    }
    for row in &original.pending.kills {
        budget.charge_work(2)?;
        let (input_block, input_gap, actual) = source_point(row.block, row.gap, budget)?;
        let Some((block, gap)) = actual else { continue; };
        let segment = segments.get(input_block.block as usize).copied().flatten()
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("logical alias kill segment"))?;
        let index = private_array_partition_v1(kills,
            |row| [row.block.0 as usize, row.gap, row.slot],
            [block.0 as usize, gap, row.slot], false,
            &mut SourceCorrespondenceWorkV18(budget))?;
        let mapped = kills.get(index).ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "logical alias kill missing mapped original row"))?;
        budget.charge_work(3)?;
        if mapped.block != block || mapped.gap != gap || mapped.slot != row.slot {
            return relation.source.missing("logical alias kill changed exact source transition");
        }
        if ordered.len() >= capacity { return relation.source.missing("logical alias extra kill event"); }
        ordered.push(OptimizedAliasBoundaryV18 {
            actual: SourceAddressBoundaryEventV29 { block, gap, kind: SourceAddressBoundaryKindV29::Kill(index) },
            order: [segment.ordinal, input_gap, 2, row.slot],
        });
    }
    // Unreachable original kills have no output gap. Every reachable occurrence
    // was joined above; the sorter separately requires every physical kill.
    let boundaries = optimized_alias_order_boundaries_v18(&mut ordered, lifetimes, kills, budget)?;
    let mut uses = emission_vec_v1(output_uses.len(), budget).map_err(immutable_memory_error_v29)?;
    for join in &output_uses {
        budget.charge_work(2)?;
        let definition = optimized_alias_use_definition_v18(relation, join.input, budget)?;
        if !input.definitions.contains(&definition) {
            return relation.source.missing("logical alias output use changed original function");
        }
        let original_definition = &relation.inventory.definitions()[definition];
        if !matches!(original_definition.ty, Type::Pointer(_)) { continue; }
        let index = definition - input.definitions.start;
        if let Some(alias) = logical[index] {
            budget.charge_work(2)?;
            if join.output.value != aliases[alias].pointer {
                return relation.source.missing("logical alias output use changed exact descendant value");
            }
            if join.output.operation.is_none() && join.output.successor.is_none() {
                return relation.source.missing("logical alias pointer-return transport remains pending");
            }
            uses.push(SourceAddressLogicalUseV29 { alias, ..join.output });
        } else if graph.exact(join.output.value, budget).map_err(immutable_memory_error_v29)?.is_some()
            && physical[index] != Some(join.output.value)
        {
            return relation.source.missing("local pointer use lacks exact checked currentness definition");
        }
    }
    private_array_heapsort_v1(&mut uses, SourceAddressLogicalUseV29::key,
        &mut SourceCorrespondenceWorkV18(budget), || ArgumentResourceV1::Arithmetic.into())?;
    for pair in uses.windows(2) {
        budget.charge_work(1)?;
        if pair[0].key() >= pair[1].key() {
            return relation.source.missing("logical alias repeated actual output use role");
        }
    }
    optimized_source_endpoints_v18(relation, optimized, budget)?;
    let owned = argument_sum_v1(&[
        size_of::<OptimizedSourceAliasTransportV18>(),
        size_of::<OptimizedSourceCurrentnessEquationsV18<'_, '_>>(),
        argument_product_v1(aliases.capacity(), size_of::<SourceAddressLogicalAliasV29>())?,
        argument_product_v1(uses.capacity(), size_of::<SourceAddressLogicalUseV29>())?,
        argument_product_v1(boundaries.capacity(), size_of::<SourceAddressBoundaryEventV29>())?,
    ])?;
    let scratch = budget.storage().checked_sub(argument_sum_v1(&[floor, owned])?)
        .ok_or(ArgumentResourceV1::Accounting)?;
    drop((segments, output_uses, original_births, recipes, logical, physical, ordered));
    budget.release_storage(scratch)?;
    Ok(OptimizedSourceAliasTransportV18 { aliases, uses, boundaries })
}

#[cfg(test)]
#[path = "production_optimized_source_alias_transport_v18_tests.rs"]
mod optimized_alias_transport_tests_v18;

#[cfg(test)]
pub(super) fn test_optimized_alias_fresh_holder_equations_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input: &fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'_, '_>,
    output: &fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    with_checked_source_memory_v29(relation, 0, Some(input), budget, |original, budget| {
        let floor = budget.storage();
        scoped_source_attempt_v29(relation.source.cleanup, budget, floor, |budget| {
            let mut challenged = 0;
            let accesses = check_optimized_source_memory_equations_v18(original, optimized, output, budget,
                |equations, transport, budget| {
                    challenged = optimized_alias_transport_tests_v18::challenge_generated_holder_roles(
                        equations, transport, budget).map_err(immutable_memory_error_v29)?;
                    Ok(())
                })?;
            let retained = budget.storage().checked_sub(floor).ok_or(ArgumentResourceV1::Accounting)?;
            drop(accesses);
            budget.release_storage(retained)?;
            Ok(challenged)
        })
    })
}

#[cfg(test)]
pub(super) fn test_optimized_alias_census_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input: &fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'_, '_>,
    output: &fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(usize, usize)> {
    with_checked_source_memory_v29(relation, 0, Some(input), budget, |original, budget| {
        let floor = budget.storage();
        scoped_source_attempt_v29(relation.source.cleanup, budget, floor, |budget| {
            let mut counts = (0, 0);
            let accesses = check_optimized_source_memory_equations_v18(original, optimized, output, budget,
                |_, transport, budget| {
                    budget.charge_work(2)?;
                    counts = (transport.aliases.len(), transport.uses.len());
                    Ok(())
                })?;
            let retained = budget.storage().checked_sub(floor).ok_or(ArgumentResourceV1::Accounting)?;
            drop(accesses);
            budget.release_storage(retained)?;
            Ok(counts)
        })
    })
}
