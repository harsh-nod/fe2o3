// Equations over the actual expanded function. Access targets are hypotheses:
// every one must equal the independently grounded physical solution afterward.
// Neither a source origin nor an initialization summary seeds these equations.
#[cfg(test)]
#[path = "production_source_reference_address_memory_v29_tests.rs"]
mod source_address_memory_tests_v29;

type SourceAddressOriginV29 = origin_worklist_v1::OriginStateV1<Option<usize>>;
include!("production_source_static_object_geometry_v29.rs");
include!("production_source_static_pointer_cells_v29.rs");

include!("production_source_reference_logical_alias_v29.rs");
include!("production_source_address_footprints_v33.rs");

#[derive(Clone, Copy)]
struct SourceAddressValueAccessV29 {
    pointer: ValueId,
    value: ValueId,
    access: MemoryAccess,
    writing: bool,
    object: bool,
}

fn source_address_value_access_v29(
    operation: &Operation,
) -> Result<Option<SourceAddressValueAccessV29>, ProductionSemanticKirErrorV1> {
    let (pointer, access, stored, object) = match operation.kind {
        OperationKind::Load { pointer, access } => (pointer, access, None, false),
        OperationKind::Store {
            pointer,
            value,
            access,
        } => (pointer, access, Some(value), false),
        OperationKind::Storage(ScopedObjectOperationV29::ReadValue { address, access }) => {
            (address, access, None, true)
        }
        OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
            address,
            value,
            access,
        }) => (address, access, Some(value), true),
        OperationKind::Storage(ScopedObjectOperationV29::Project {
            step:
                ScopedObjectProjectionV29::Field(_)
                | ScopedObjectProjectionV29::ArrayIndex(_)
                | ScopedObjectProjectionV29::VariantForWrite { .. },
            ..
        }) => return Ok(None),
        OperationKind::Storage(
            ScopedObjectOperationV29::ReadDiscriminant { .. }
            | ScopedObjectOperationV29::SetDiscriminant { .. },
        ) => return Ok(None),
        OperationKind::Storage(_) => return Err(scoped_object_pending_v29()),
        _ => return Ok(None),
    };
    let value = match (stored, operation.results.as_slice()) {
        (Some(value), []) => value,
        (None, [result]) => result.id,
        _ => return Err(source_raw_physical_error_v29()),
    };
    Ok(Some(SourceAddressValueAccessV29 {
        pointer,
        value,
        access,
        writing: stored.is_some(),
        object,
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceAddressAccessV29 {
    block: BlockId,
    operation: usize,
    footprint: u32,
    slot: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceAddressKillV29 {
    block: BlockId,
    gap: usize,
    slot: usize,
    // Invocation resets precede source events. Other kills retain the exact
    // original anchor order when no physical operation separates boundaries.
    // Checked output segment, original input gap, reset/source, instance, anchor.
    source_order: [usize; 5],
}

impl SourceAddressKillV29 {
    fn key_v45(&self) -> [usize; 8] {
        let [segment, gap, phase, instance, anchor] = self.source_order;
        [
            self.block.0 as usize,
            self.gap,
            segment,
            gap,
            phase,
            instance,
            anchor,
            self.slot,
        ]
    }
}

type SourceAddressKillOrderFrameV45<'a> = (
    Option<(BlockId, usize, [usize; 5], usize)>,
    (BlockId, usize, [usize; 5], usize),
    &'a SourceAddressKillV29,
);

// Original-source mapping supplies these ordered lifetime boundaries. Repeating
// a `live` boundary always expires old aliases before the new activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceAddressLifetimeV29 {
    block: BlockId,
    gap: usize,
    sequence: usize,
    source_order: [usize; 5],
    slot: usize,
    live: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceAddressBirthV29 {
    block: BlockId,
    operation: usize,
    result: ValueId,
}

struct SourceAddressCurrentnessV29<'graph, 'kir> {
    graph: &'graph SourceAddressMemoryV29<'kir>,
    registers: Vec<Option<usize>>,
    objects: Vec<(usize, bool)>,
    width: usize,
    nodes: usize,
    entry: usize,
    transport: SourceAddressAliasTransportV29<'graph>,
}

impl SourceAddressCurrentnessV29<'_, '_> {
    fn state(
        &self,
        block: usize,
        component: usize,
        output: bool,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        if block >= self.graph.blocks.len() || component >= self.width {
            return Err(source_raw_physical_error_v29());
        }
        argument_sum_v1(&[
            argument_product_v1(
                argument_sum_v1(&[argument_product_v1(block, 2)?, usize::from(output)])?,
                self.width,
            )?,
            component,
        ])
        .map_err(Into::into)
    }

    fn definition(&self, register: usize) -> Result<usize, ProductionSemanticKirErrorV1> {
        if register >= self.objects.len() {
            return Err(source_raw_physical_error_v29());
        }
        argument_sum_v1(&[
            argument_product_v1(argument_product_v1(self.graph.blocks.len(), 2)?, self.width)?,
            register,
        ])
        .map_err(Into::into)
    }

    fn constant(&self, value: bool) -> usize {
        self.nodes - if value { 2 } else { 1 }
    }

    fn object(&self, slot: usize) -> Result<usize, ProductionSemanticKirErrorV1> {
        argument_sum_v1(&[self.objects.len(), self.graph.pointer_cell_count, slot])
            .map_err(Into::into)
    }

    fn register(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        Ok(self.registers[self.graph.value(value, budget)?])
    }

    fn logical_register(&self, alias: usize) -> Result<usize, ProductionSemanticKirErrorV1> {
        if alias >= self.transport.aliases.len() {
            return Err(source_raw_physical_error_v29());
        }
        self.objects
            .len()
            .checked_sub(self.transport.aliases.len())
            .and_then(|start| start.checked_add(alias))
            .ok_or_else(source_raw_physical_error_v29)
    }

    fn alias_input_register(
        &self,
        input: SourceAddressAliasInputV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        match input {
            SourceAddressAliasInputV29::Physical(value) => self
                .register(value, budget)?
                .ok_or_else(source_raw_physical_error_v29),
            SourceAddressAliasInputV29::Logical(alias) => self.logical_register(alias),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn use_register(
        &self,
        block: BlockId,
        operation: Option<usize>,
        successor: Option<usize>,
        operand: usize,
        value: ValueId,
        visited: &mut usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        if self.transport.uses.is_empty() {
            return self.register(value, budget);
        }
        let key = [
            block.0 as usize,
            operation.unwrap_or(usize::MAX),
            successor.unwrap_or(usize::MAX),
            operand,
        ];
        budget.charge_work(call_splice_search_work_v1(self.transport.uses.len()))?;
        if let Ok(index) = self
            .transport
            .uses
            .binary_search_by_key(&key, SourceAddressLogicalUseV29::key)
        {
            budget.charge_work(2)?;
            let row = &self.transport.uses[index];
            if row.value != value {
                return Err(source_raw_physical_error_v29());
            }
            *visited = argument_sum_v1(&[*visited, 1])?;
            return Ok(Some(self.logical_register(row.alias)?));
        }
        self.register(value, budget)
    }

    // Equations retain dynamic value validity separately from the physical
    // object origin. This is the existing solver on the actual graph, not a
    // second source CFG interpreter or a static-site incarnation counter.
    #[allow(clippy::too_many_arguments)]
    fn equations(
        &self,
        accesses: &[SourceAddressAccessV29],
        kills: &[SourceAddressKillV29],
        lifetimes: &[SourceAddressLifetimeV29],
        births: &[SourceAddressBirthV29],
        failures: &[SourceIndexFailureV29],
        geometry: SourceAddressGeometryV29,
        budget: &mut ArgumentBudgetV1<'_>,
        mut link: impl FnMut(
            usize,
            usize,
            &mut ArgumentBudgetV1<'_>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
        mut check: impl FnMut(
            usize,
            &mut ArgumentBudgetV1<'_>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut current = emission_vec_v1(self.width, budget)?;
        let mut cell_origins = emission_vec_v1(self.graph.pointer_cell_count, budget)?;
        let mut edge_arguments = emission_vec_v1(self.objects.len(), budget)?;
        budget.charge_work(argument_sum_v1(&[
            self.width,
            self.graph.pointer_cell_count,
            self.objects.len(),
        ])?)?;
        current.resize(self.width, self.constant(false));
        cell_origins.resize(self.graph.pointer_cell_count, self.graph.unknown());
        edge_arguments.resize(self.objects.len(), None);
        let scratch = argument_sum_v1(&[
            argument_product_v1(current.capacity(), std::mem::size_of::<usize>())?,
            argument_product_v1(cell_origins.capacity(), std::mem::size_of::<usize>())?,
            argument_product_v1(
                edge_arguments.capacity(),
                std::mem::size_of::<Option<usize>>(),
            )?,
        ])?;
        let mut boundaries = SourceAddressBoundaryCursorV29::default();
        let (mut next_birth, mut next_failure, mut used_aliases) = (0, 0, 0);
        for (block_index, (_, block)) in self.graph.blocks.iter().enumerate() {
            budget.charge_work(argument_sum_v1(&[
                self.width,
                self.graph.pointer_cell_count,
                1,
            ])?)?;
            for (component, value) in current.iter_mut().enumerate() {
                *value = self.state(block_index, component, false)?;
            }
            for (cell, value) in cell_origins.iter_mut().enumerate() {
                *value = self.graph.memory(block_index, cell, false)?;
            }
            for gap in 0..=block.operations.len() {
                budget.charge_work(1)?;
                while let Some(event) =
                    boundaries.next_at(block.id, gap, lifetimes, kills, self.transport)
                {
                    match event {
                        SourceAddressBoundaryKindV29::Lifetime(index) => {
                            let row = &lifetimes[index];
                            // A Move read precedes its range clear and any later
                            // storage marker, even when they share one MIR gap.
                            while let Some(read) = failures.get(next_failure)
                                && (read.block, read.gap) == (block.id, gap)
                            {
                                budget.charge_work(5)?;
                                if read.source_order >= row.source_order {
                                    if read.source_order == row.source_order {
                                        return Err(source_raw_physical_error_v29());
                                    }
                                    break;
                                }
                                budget.charge_work(1)?;
                                check(current[self.object(read.slot)?], budget)?;
                                next_failure += 1;
                            }
                            budget.charge_work(argument_sum_v1(&[
                                self.objects.len(),
                                self.graph.pointer_cell_count,
                                2,
                            ])?)?;
                            current[self.object(row.slot)?] = self.constant(row.live);
                            for (register, &(object, backing)) in self.objects.iter().enumerate() {
                                if object == row.slot {
                                    current[register] = self.constant(backing && row.live);
                                }
                            }
                            for (cell, &origin) in cell_origins.iter().enumerate() {
                                // An unresolved union of objects cannot establish that
                                // this stored alias excludes the ended activation.
                                if !matches!(self.graph.origins[origin], SourceAddressOriginV29::Exact(object)
                            if object != Some(row.slot))
                                {
                                    current[self.objects.len() + cell] = self.constant(false);
                                }
                            }
                            for cell in self.graph.pointer_cells[row.slot]
                                .into_iter()
                                .chain(self.graph.pointer_cell_range(row.slot, budget)?)
                            {
                                current[self.objects.len() + cell] = self.constant(false);
                                cell_origins[cell] = self.graph.unknown();
                            }
                        }
                        SourceAddressBoundaryKindV29::Kill(index) => {
                            let row = &kills[index];
                            budget.charge_work(2)?;
                            for cell in self.graph.pointer_cells[row.slot]
                                .into_iter()
                                .chain(self.graph.pointer_cell_range(row.slot, budget)?)
                            {
                                current[self.objects.len() + cell] = self.constant(false);
                                cell_origins[cell] = self.graph.unknown();
                            }
                        }
                        SourceAddressBoundaryKindV29::Alias(index) => {
                            budget.charge_work(2)?;
                            let register = self.logical_register(index)?;
                            let alias = &self.transport.aliases[index];
                            let definition = self.definition(register)?;
                            match alias.recipe {
                                SourceAddressAliasRecipeV29::Birth => {
                                    link(current[self.object(alias.slot)?], definition, budget)?
                                }
                                SourceAddressAliasRecipeV29::Inherit(input) => {
                                    let input = self.alias_input_register(input, budget)?;
                                    link(current[input], definition, budget)?;
                                }
                                SourceAddressAliasRecipeV29::Select(left, right) => {
                                    for input in [left, right] {
                                        let input = self.alias_input_register(input, budget)?;
                                        link(current[input], definition, budget)?;
                                    }
                                }
                            }
                            current[register] = definition;
                        }
                    }
                }
                let Some(operation) = block.operations.get(gap) else {
                    while let Some(row) = failures.get(next_failure)
                        && (row.block, row.gap) == (block.id, gap)
                    {
                        budget.charge_work(1)?;
                        check(current[self.object(row.slot)?], budget)?;
                        next_failure += 1;
                    }
                    continue;
                };
                while let Some(row) = failures.get(next_failure)
                    && (row.block, row.gap) == (block.id, gap)
                {
                    budget.charge_work(1)?;
                    check(current[self.object(row.slot)?], budget)?;
                    next_failure += 1;
                }
                let access = SourceAddressMemoryV29::access(accesses, block.id, gap, budget)?;
                if let Some(reference) = self
                    .graph
                    .compiler_reference_use_v55(block.id, gap, operation, budget)?
                {
                    let register = self
                        .use_register(
                            block.id,
                            Some(gap),
                            None,
                            1,
                            reference.value,
                            &mut used_aliases,
                            budget,
                        )?
                        .ok_or_else(source_raw_physical_error_v29)?;
                    check(current[register], budget)?;
                    check(current[self.object(reference.custody.slot)?], budget)?;
                }
                if let Some(access) = access {
                    let pointer = if let Some(tag) = source_address_tag_access_v43(operation)? {
                        tag.pointer
                    } else {
                        source_address_value_access_v29(operation)?
                            .ok_or_else(source_raw_physical_error_v29)?
                            .pointer
                    };
                    let register = self
                        .use_register(
                            block.id,
                            Some(gap),
                            None,
                            0,
                            pointer,
                            &mut used_aliases,
                            budget,
                        )?
                        .ok_or_else(source_raw_physical_error_v29)?;
                    check(current[register], budget)?;
                    check(current[self.object(access.slot)?], budget)?;
                }
                let project_input =
                    if let OperationKind::Storage(ScopedObjectOperationV29::Project {
                        base, ..
                    }) = operation.kind
                    {
                        let input = self
                            .use_register(
                                block.id,
                                Some(gap),
                                None,
                                0,
                                base,
                                &mut used_aliases,
                                budget,
                            )?
                            .ok_or_else(source_raw_physical_error_v29)?;
                        check(current[input], budget)?;
                        check(current[self.object(self.objects[input].0)?], budget)?;
                        Some(input)
                    } else {
                        None
                    };
                let birth = births
                    .get(next_birth)
                    .filter(|row| (row.block, row.operation) == (block.id, gap));
                if let Some(birth) = birth {
                    let register = self
                        .register(birth.result, budget)?
                        .ok_or_else(source_raw_physical_error_v29)?;
                    link(
                        current[self.object(self.objects[register].0)?],
                        self.definition(register)?,
                        budget,
                    )?;
                    current[register] = self.definition(register)?;
                    next_birth += 1;
                } else if let [result] = operation.results.as_slice()
                    && let Some(register) = self.register(result.id, budget)?
                {
                    let definition = self.definition(register)?;
                    match &operation.kind {
                        OperationKind::Alloca { .. } => link(
                            current[self.object(self.objects[register].0)?],
                            definition,
                            budget,
                        )?,
                        OperationKind::Cast {
                            kind: CastKind::RestrictPointerAccess | CastKind::PointerToGeneric,
                            value,
                            ..
                        } => {
                            let input = self
                                .use_register(
                                    block.id,
                                    Some(gap),
                                    None,
                                    0,
                                    *value,
                                    &mut used_aliases,
                                    budget,
                                )?
                                .ok_or_else(source_raw_physical_error_v29)?;
                            link(current[input], definition, budget)?;
                        }
                        OperationKind::GetElementPointer { base, .. } => {
                            match geometry {
                                SourceAddressGeometryV29::Scalar => {
                                    self.graph.check_zero_gep(operation, budget)?
                                }
                                SourceAddressGeometryV29::PendingIndices => {
                                    self.graph.check_gep_type(operation, budget)?
                                }
                            }
                            let input = self
                                .use_register(
                                    block.id,
                                    Some(gap),
                                    None,
                                    0,
                                    *base,
                                    &mut used_aliases,
                                    budget,
                                )?
                                .ok_or_else(source_raw_physical_error_v29)?;
                            link(current[input], definition, budget)?;
                        }
                        OperationKind::Storage(ScopedObjectOperationV29::Project { .. }) => {
                            let input = project_input.ok_or_else(source_raw_physical_error_v29)?;
                            link(current[input], definition, budget)?;
                        }
                        OperationKind::Select {
                            true_value,
                            false_value,
                            ..
                        } => {
                            for (operand, value) in [(1, true_value), (2, false_value)] {
                                let input = self
                                    .use_register(
                                        block.id,
                                        Some(gap),
                                        None,
                                        operand,
                                        *value,
                                        &mut used_aliases,
                                        budget,
                                    )?
                                    .ok_or_else(source_raw_physical_error_v29)?;
                                link(current[input], definition, budget)?;
                            }
                        }
                        OperationKind::Load { .. }
                        | OperationKind::Storage(ScopedObjectOperationV29::ReadValue { .. }) => {
                            let cell = self
                                .graph
                                .access_pointer_cell(access, operation, budget)?
                                .ok_or_else(source_raw_physical_error_v29)?;
                            link(current[self.objects.len() + cell], definition, budget)?;
                        }
                        _ => return Err(source_raw_physical_error_v29()),
                    }
                    current[register] = definition;
                }
                if let Some(SourceAddressValueAccessV29 {
                    value,
                    writing: true,
                    ..
                }) = source_address_value_access_v29(operation)?
                    && let Some(cell) = self.graph.access_pointer_cell(access, operation, budget)?
                {
                    current[self.objects.len() + cell] = match self.use_register(
                        block.id,
                        Some(gap),
                        None,
                        1,
                        value,
                        &mut used_aliases,
                        budget,
                    )? {
                        Some(register) => current[register],
                        None => self.constant(true),
                    };
                    cell_origins[cell] = self.graph.value(value, budget)?;
                }
                if let Some(tag) = source_address_tag_access_v43(operation)? {
                    let row = access.ok_or_else(source_raw_physical_error_v29)?;
                    self.graph.visit_tag_overwritten_pointer_cells_v43(
                        tag,
                        row.slot,
                        budget,
                        |cell, _| {
                            current[self.objects.len() + cell] = self.constant(false);
                            cell_origins[cell] = self.graph.unknown();
                            Ok(())
                        },
                    )?;
                }
            }
            for (component, &value) in current.iter().enumerate() {
                budget.charge_work(1)?;
                link(value, self.state(block_index, component, true)?, budget)?;
            }
            let mut successor = 0;
            block
                .terminator
                .as_ref()
                .ok_or_else(source_raw_physical_error_v29)?
                .try_visit_edges_v1(|target, arguments| {
                    let target_index = self.graph.block(target, budget)?;
                    if target_index == self.entry {
                        return Err(source_raw_physical_error_v29());
                    }
                    let target = self.graph.blocks[target_index].1;
                    if target.parameters.len() != arguments.len() {
                        return Err(source_raw_physical_error_v29());
                    }
                    budget.charge_work(argument_sum_v1(&[
                        edge_arguments.len(),
                        arguments.len(),
                        self.width,
                    ])?)?;
                    edge_arguments.fill(None);
                    for (operand, (parameter, &argument)) in
                        target.parameters.iter().zip(arguments).enumerate()
                    {
                        if let Some(register) = self.register(parameter.id, budget)? {
                            let source = self
                                .use_register(
                                    block.id,
                                    None,
                                    Some(successor),
                                    operand,
                                    argument,
                                    &mut used_aliases,
                                    budget,
                                )?
                                .ok_or_else(source_raw_physical_error_v29)?;
                            edge_arguments[register] = Some(current[source]);
                        }
                    }
                    for component in 0..current.len() {
                        let value = match edge_arguments.get(component).copied().flatten() {
                            Some(value) => value,
                            None => self.state(block_index, component, true)?,
                        };
                        link(value, self.state(target_index, component, false)?, budget)?;
                    }
                    successor = argument_sum_v1(&[successor, 1])?;
                    Ok::<_, ProductionSemanticKirErrorV1>(())
                })?;
        }
        if !boundaries.complete(lifetimes.len(), kills.len(), self.transport)
            || used_aliases != self.transport.uses.len()
            || next_birth != births.len()
            || next_failure != failures.len()
        {
            return Err(source_raw_physical_error_v29());
        }
        drop((current, cell_origins, edge_arguments));
        budget.release_storage(scratch)?;
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn check_source_address_currentness_v29(
    function: &Function,
    graph: &SourceAddressMemoryV29<'_>,
    slots: &[ScopedSourceSlotV29],
    accesses: &[SourceAddressAccessV29],
    kills: &[SourceAddressKillV29],
    initial_live: &[bool],
    lifetimes: &[SourceAddressLifetimeV29],
    births: &[SourceAddressBirthV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_source_address_currentness_geometry_v29(
        function,
        graph,
        slots,
        accesses,
        kills,
        initial_live,
        lifetimes,
        births,
        &[],
        SourceAddressGeometryV29::Scalar,
        budget,
    )
}

#[allow(clippy::too_many_arguments)]
fn check_source_address_currentness_geometry_v29(
    function: &Function,
    graph: &SourceAddressMemoryV29<'_>,
    slots: &[ScopedSourceSlotV29],
    accesses: &[SourceAddressAccessV29],
    kills: &[SourceAddressKillV29],
    initial_live: &[bool],
    lifetimes: &[SourceAddressLifetimeV29],
    births: &[SourceAddressBirthV29],
    failures: &[SourceIndexFailureV29],
    geometry: SourceAddressGeometryV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_source_address_currentness_transport_v29(
        function,
        graph,
        slots,
        accesses,
        kills,
        initial_live,
        lifetimes,
        births,
        failures,
        geometry,
        SourceAddressAliasTransportV29::default(),
        budget,
    )
}

fn source_address_currentness_headers_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<SourceAddressCurrentnessV29<'_, '_>>(),
        std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
        argument_product_v1(4, std::mem::size_of::<Vec<usize>>())?,
        std::mem::size_of::<SourceAddressBoundaryCursorV29>(),
        std::mem::size_of::<[usize; 4]>(),
        std::mem::size_of::<Result<usize, usize>>(),
        std::mem::size_of::<Option<usize>>(),
        std::mem::size_of::<Option<(BlockId, usize, [usize; 5])>>(),
        std::mem::size_of::<(BlockId, usize, [usize; 5])>(),
        std::mem::size_of::<Option<[usize; 7]>>(),
        std::mem::size_of::<[usize; 7]>(),
    ])
}

fn source_address_currentness_check_count_v34(
    accesses: usize,
    projects: usize,
    failures: usize,
) -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        argument_product_v1(accesses, 2)?,
        argument_product_v1(projects, 2)?,
        failures,
    ])
}

#[allow(clippy::too_many_arguments)]
fn check_source_address_currentness_transport_v29(
    function: &Function,
    graph: &SourceAddressMemoryV29<'_>,
    slots: &[ScopedSourceSlotV29],
    accesses: &[SourceAddressAccessV29],
    kills: &[SourceAddressKillV29],
    initial_live: &[bool],
    lifetimes: &[SourceAddressLifetimeV29],
    births: &[SourceAddressBirthV29],
    failures: &[SourceIndexFailureV29],
    geometry: SourceAddressGeometryV29,
    transport: SourceAddressAliasTransportV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot_identity = std::ptr::from_ref(budget) as usize;
    budget.reserve_storage(source_address_currentness_headers_v29()?)?;
    if initial_live.len() != slots.len() || graph.pointer_cells.len() != slots.len() {
        return Err(source_raw_physical_error_v29());
    }
    let body = function
        .body
        .as_ref()
        .ok_or_else(source_raw_physical_error_v29)?;
    let entry = graph.block(
        body.blocks
            .first()
            .ok_or_else(source_raw_physical_error_v29)?
            .id,
        budget,
    )?;
    // The graph borrower, not a similarly shaped candidate, defines this pass.
    budget.charge_work(1)?;
    if body.blocks.len() != graph.blocks.len() {
        return Err(source_raw_physical_error_v29());
    }
    for block in &body.blocks {
        if !std::ptr::eq(graph.blocks[graph.block(block.id, budget)?].1, block) {
            return Err(source_raw_physical_error_v29());
        }
    }
    graph.validate_coordinates(slots, accesses, kills, budget)?;
    graph.validate_solution(slots, accesses, geometry, budget)?;
    transport.validate(graph, slots, lifetimes, kills, budget)?;
    let mut registers = emission_vec_v1(graph.index.values.len(), budget)?;
    let mut objects = emission_vec_v1(
        argument_sum_v1(&[graph.index.values.len(), transport.aliases.len()])?,
        budget,
    )?;
    for (index, (value, _)) in graph.index.values.iter().enumerate() {
        budget.charge_work(2)?;
        if let SourceAddressOriginV29::Exact(Some(slot)) = graph.origins[index] {
            let original = slots.get(slot).ok_or_else(source_raw_physical_error_v29)?;
            registers.push(Some(objects.len()));
            objects.push((slot, *value == original.origin.pointer));
        } else {
            registers.push(None);
        }
    }
    for alias in transport.aliases {
        budget.charge_work(1)?;
        objects.push((alias.slot, false));
    }
    let width = argument_sum_v1(&[objects.len(), graph.pointer_cell_count, slots.len()])?;
    let nodes = argument_sum_v1(&[
        argument_product_v1(argument_product_v1(graph.blocks.len(), 2)?, width)?,
        objects.len(),
        2,
    ])?;
    let currentness = SourceAddressCurrentnessV29 {
        graph,
        registers,
        objects,
        width,
        nodes,
        entry,
        transport,
    };
    let mut previous = None;
    let mut previous_order = None;
    for row in lifetimes {
        budget.charge_work(8)?;
        let key = (row.block, row.gap, row.sequence);
        let order = (row.block, row.gap, row.source_order);
        if previous.is_some_and(|previous| previous >= key)
            || previous_order.is_some_and(|previous| previous > order)
            || row.slot >= slots.len()
            || row.gap
                > graph.blocks[graph.block(row.block, budget)?]
                    .1
                    .operations
                    .len()
        {
            return Err(source_raw_physical_error_v29());
        }
        previous = Some(key);
        previous_order = Some(order);
    }
    let mut previous = None;
    for row in births {
        budget.charge_work(4)?;
        let key = (row.block, row.operation);
        let operation = graph.blocks[graph.block(row.block, budget)?]
            .1
            .operations
            .get(row.operation)
            .ok_or_else(source_raw_physical_error_v29)?;
        let register = currentness
            .register(row.result, budget)?
            .ok_or_else(source_raw_physical_error_v29)?;
        if previous.is_some_and(|previous| previous >= key)
            || currentness.objects[register].1
            || !matches!(operation.results.as_slice(), [result] if result.id == row.result)
        {
            return Err(source_raw_physical_error_v29());
        }
        match operation.kind {
            OperationKind::GetElementPointer { .. } => graph.check_zero_gep(operation, budget)?,
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric | CastKind::RestrictPointerAccess,
                ..
            } => {}
            OperationKind::Select {
                condition,
                true_value,
                false_value,
            } => {
                // Only the authenticated Object formation may refresh an alias.
                // Selecting a saved alias, even twice, must not revive it.
                let slot = &slots[currentness.objects[register].0];
                let ScopedSlotRepresentationV29::Object { schema, .. } = slot.representation else {
                    return Err(source_raw_physical_error_v29());
                };
                budget.charge_work(4)?;
                if true_value != slot.origin.pointer || false_value != slot.origin.pointer {
                    return Err(source_raw_physical_error_v29());
                }
                let Type::Pointer(pointer) = graph.ty(row.result, budget)? else {
                    return Err(source_raw_physical_error_v29());
                };
                if pointer.address_space != AddressSpace::Private
                    || pointer.access != AccessMode::ReadWrite
                    || pointer.pointee.as_ref() != &Type::StorageObject(schema)
                {
                    return Err(source_raw_physical_error_v29());
                }
                SourceAddressMemoryV29::same_type(
                    graph.ty(condition, budget)?,
                    &Type::BOOL,
                    budget,
                )?;
                SourceAddressMemoryV29::same_type(
                    graph.ty(true_value, budget)?,
                    graph.ty(row.result, budget)?,
                    budget,
                )?;
            }
            _ => return Err(source_raw_physical_error_v29()),
        }
        previous = Some(key);
    }
    let mut links = 0;
    let mut previous = None;
    for row in failures {
        budget.charge_work(7)?;
        let key = row.key_v45();
        if previous.is_some_and(|prior| prior >= key)
            || row.slot >= slots.len()
            || row.gap
                > graph.blocks[graph.block(row.block, budget)?]
                    .1
                    .operations
                    .len()
        {
            return Err(source_raw_physical_error_v29());
        }
        previous = Some(key);
    }
    currentness.equations(
        accesses,
        kills,
        lifetimes,
        births,
        failures,
        geometry,
        budget,
        |_, _, _| {
            links = argument_sum_v1(&[links, 1])?;
            Ok(())
        },
        |_, _| Ok(()),
    )?;
    let mut solver = origin_worklist_v1::OriginWorkV1::<bool>::new(nodes, links, budget)
        .map_err(source_address_equation_error_v29)?;
    for node in 0..nodes {
        let seed = if node == currentness.constant(true) {
            origin_worklist_v1::OriginStateV1::Exact(true)
        } else if node == currentness.constant(false) {
            origin_worklist_v1::OriginStateV1::Exact(false)
        } else {
            let start = currentness.state(entry, 0, false)?;
            if node >= start && node - start < width {
                let component = node - start;
                let live = if component < currentness.objects.len() {
                    let (object, backing) = currentness.objects[component];
                    backing && initial_live[object]
                } else if component < currentness.objects.len() + graph.pointer_cell_count {
                    false
                } else {
                    initial_live[component - currentness.objects.len() - graph.pointer_cell_count]
                };
                origin_worklist_v1::OriginStateV1::Exact(live)
            } else {
                origin_worklist_v1::OriginStateV1::Pending
            }
        };
        solver
            .seed_next(seed, budget)
            .map_err(source_address_equation_error_v29)?;
    }
    // Zero-footprint Projects still check both the base alias and its live
    // object. Their complete actual graph census must own both check rows.
    let capacity = source_address_currentness_check_count_v34(
        argument_sum_v1(&[accesses.len(), graph.compiler_references.len()])?,
        graph.projections.len(),
        failures.len(),
    )?;
    let mut checks = emission_vec_v1(capacity, budget)?;
    currentness.equations(
        accesses,
        kills,
        lifetimes,
        births,
        failures,
        geometry,
        budget,
        |source, target, budget| {
            solver
                .add_link(source, target, budget)
                .map_err(source_address_equation_error_v29)
        },
        |node, budget| {
            budget.charge_work(1)?;
            if checks.len() >= capacity || checks.len() == checks.capacity() {
                return Err(source_raw_physical_error_v29());
            }
            checks.push(node);
            Ok(())
        },
    )?;
    if checks.len() != capacity {
        return Err(source_raw_physical_error_v29());
    }
    let solution = solver
        .solve(budget)
        .map_err(source_address_equation_error_v29)?;
    for &node in &checks {
        budget.charge_work(1)?;
        if solution.get(node) != Some(&origin_worklist_v1::OriginStateV1::Exact(true)) {
            return Err(source_reference_error_v29(
                "physical raw access crosses a storage activation or unresolved alias",
            ));
        }
    }
    drop((solution, checks, currentness));
    // Closed primitive scratch only: no callback, C2 mutation, source-plan
    // allocation or retained result runs in this pass. The owner scope still
    // performs its concrete growth-aware settlement on every failure.
    if ledger != budget.work_ledger_identity_v1()
        || slot_identity != std::ptr::from_ref(budget) as usize
    {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let scratch = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    budget.release_storage(scratch)?;
    Ok(())
}

struct SourceAddressMemoryV29<'kir> {
    index: CallSpliceIndexV1<'kir>,
    blocks: Vec<(BlockId, &'kir BasicBlock)>,
    pointer_cells: Vec<Option<usize>>,
    pointer_cell_count: usize,
    pointer_subcells: Vec<SourceStaticPointerCellV29>,
    pointer_subcell_addresses: Vec<(ValueId, usize)>,
    object_layouts: Vec<SourceStaticObjectLayoutV29>,
    object_schemas: Vec<Option<fe2o3_kernel_ir::StorageLayoutIdV1>>,
    zero_offsets: Vec<bool>,
    origins: Vec<SourceAddressOriginV29>,
    projections: Vec<(usize, SourceStaticObjectTransferV29)>,
    locations: Vec<origin_worklist_v1::OriginStateV1<Option<SourceStaticObjectLocationV29>>>,
    compiler_references: Vec<SourceCompilerEnumReferenceUseV55>,
}

include!("production_compiler_enum_reference_uses_v55.rs");

// This owner contains solved object-origin equations, not checked array
// geometry. Only the source-index census consumes its outstanding ranges.
struct PendingSourceIndexPhysicalV29<'kir> {
    graph: SourceAddressMemoryV29<'kir>,
}

#[derive(Clone, Copy)]
enum SourceAddressGeometryV29 {
    Scalar,
    PendingIndices,
}

fn source_address_equation_error_v29(
    error: origin_worklist_v1::OriginWorkErrorV1,
) -> ProductionSemanticKirErrorV1 {
    match error {
        origin_worklist_v1::OriginWorkErrorV1::Resource(error) => error.into(),
        origin_worklist_v1::OriginWorkErrorV1::Shape => source_raw_physical_error_v29(),
    }
}

fn source_address_call_error_v29(
    error: CallInstanceEmissionErrorV1,
) -> ProductionSemanticKirErrorV1 {
    match error {
        CallInstanceEmissionErrorV1::Resource(error) => error.into(),
        _ => source_raw_physical_error_v29(),
    }
}

impl<'kir> SourceAddressMemoryV29<'kir> {
    fn value(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(call_splice_search_work_v1(self.index.values.len()))?;
        self.index
            .values
            .binary_search_by_key(&value, |(id, _)| *id)
            .map_err(|_| source_raw_physical_error_v29())
    }

    fn ty(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'kir Type, ProductionSemanticKirErrorV1> {
        Ok(self.index.values[self.value(value, budget)?].1)
    }

    fn block(
        &self,
        block: BlockId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(call_splice_search_work_v1(self.blocks.len()))?;
        self.blocks
            .binary_search_by_key(&block, |(id, _)| *id)
            .map_err(|_| source_raw_physical_error_v29())
    }

    fn memory(
        &self,
        block: usize,
        cell: usize,
        output: bool,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        if block >= self.blocks.len() || cell >= self.pointer_cell_count {
            return Err(source_raw_physical_error_v29());
        }
        argument_sum_v1(&[
            self.index.values.len(),
            argument_product_v1(
                argument_sum_v1(&[argument_product_v1(block, 2)?, usize::from(output)])?,
                self.pointer_cell_count,
            )?,
            cell,
        ])
        .map_err(Into::into)
    }

    fn unknown(&self) -> usize {
        self.origins.len() - 1
    }

    fn exact(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        match self.origins[self.value(value, budget)?] {
            SourceAddressOriginV29::Exact(slot) => Ok(slot),
            SourceAddressOriginV29::Pending | SourceAddressOriginV29::Unknown => {
                Err(source_raw_physical_error_v29())
            }
        }
    }

    fn access<'a>(
        rows: &'a [SourceAddressAccessV29],
        block: BlockId,
        operation: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<&'a SourceAddressAccessV29>, ProductionSemanticKirErrorV1> {
        budget.charge_work(call_splice_search_work_v1(rows.len()))?;
        let index = rows.partition_point(|row| (row.block, row.operation) < (block, operation));
        budget.charge_work(2)?;
        let Some(row) = rows
            .get(index)
            .filter(|row| (row.block, row.operation) == (block, operation))
        else {
            return Ok(None);
        };
        if row.footprint != 0
            || rows
                .get(index + 1)
                .is_some_and(|next| (next.block, next.operation) == (block, operation))
        {
            return Err(source_raw_physical_error_v29());
        }
        Ok(Some(row))
    }

    fn access_footprint<'a>(
        rows: &'a [SourceAddressAccessV29],
        block: BlockId,
        operation: usize,
        footprint: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<&'a SourceAddressAccessV29>, ProductionSemanticKirErrorV1> {
        budget.charge_work(call_splice_search_work_v1(rows.len()))?;
        let key = (block, operation, footprint);
        let index = rows.partition_point(|row| (row.block, row.operation, row.footprint) < key);
        budget.charge_work(2)?;
        let Some(row) = rows
            .get(index)
            .filter(|row| (row.block, row.operation, row.footprint) == key)
        else {
            return Ok(None);
        };
        if rows
            .get(index + 1)
            .is_some_and(|row| (row.block, row.operation, row.footprint) == key)
        {
            return Err(source_raw_physical_error_v29());
        }
        Ok(Some(row))
    }

    fn same_type(
        left: &Type,
        right: &Type,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !call_splice_type_eq_v1(left, right, budget).map_err(source_address_call_error_v29)? {
            return Err(source_raw_physical_error_v29());
        }
        Ok(())
    }

    fn new(
        function: &'kir Function,
        slots: &[ScopedSourceSlotV29],
        parts: Option<&ScopedDeferredScalarViewV29<'_, '_, '_>>,
        accesses: &[SourceAddressAccessV29],
        kills: &[SourceAddressKillV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::prepare(function, slots, parts, accesses, budget)?
            .solve(slots, accesses, kills, budget)
    }

    fn prepare(
        function: &'kir Function,
        slots: &[ScopedSourceSlotV29],
        parts: Option<&ScopedDeferredScalarViewV29<'_, '_, '_>>,
        accesses: &[SourceAddressAccessV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::prepare_with_layouts(function, slots, parts, accesses, &[], budget)
    }

    fn prepare_with_layouts(
        function: &'kir Function,
        slots: &[ScopedSourceSlotV29],
        parts: Option<&ScopedDeferredScalarViewV29<'_, '_, '_>>,
        accesses: &[SourceAddressAccessV29],
        layouts: &[fe2o3_kernel_ir::StorageLayoutV1],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::prepare_with_compiler(function, slots, parts, accesses, layouts, None, budget)
    }

    fn prepare_with_compiler(
        function: &'kir Function,
        slots: &[ScopedSourceSlotV29],
        parts: Option<&ScopedDeferredScalarViewV29<'_, '_, '_>>,
        accesses: &[SourceAddressAccessV29],
        layouts: &[fe2o3_kernel_ir::StorageLayoutV1],
        compiler: Option<&scoped_raw_admission_v29::CheckedCompilerEnumMemoryV55<'_>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let mut scratch = 0;
        let index =
            call_splice_index_with_deferred_parts_v29(function, parts, budget, &mut scratch)
                .map_err(|error| match error {
                    CallInstanceEmissionErrorV1::Resource(error) => error.into(),
                    _ => source_raw_physical_error_v29(),
                })?;
        call_splice_check_body_v1(function, &index, false, budget).map_err(
            |error| match error {
                CallInstanceEmissionErrorV1::Resource(error) => error.into(),
                _ => source_raw_physical_error_v29(),
            },
        )?;
        Self::prepare_indexed(function, slots, accesses, index, layouts, compiler, budget)
    }

    // Final lifecycle operations are already part of this verified inventory.
    // The pre-lifecycle splicer must continue rejecting unbound Execution IR.
    fn prepare_inventory(
        inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'kir>,
        coordinate: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
        slots: &[ScopedSourceSlotV29],
        accesses: &[SourceAddressAccessV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::prepare_inventory_with_compiler(inventory, coordinate, slots, accesses, None, budget)
    }

    fn prepare_inventory_with_compiler(
        inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'kir>,
        coordinate: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
        slots: &[ScopedSourceSlotV29],
        accesses: &[SourceAddressAccessV29],
        compiler: Option<&scoped_raw_admission_v29::CheckedCompilerEnumMemoryV55<'_>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let function = inventory
            .functions()
            .get(coordinate.0 as usize)
            .filter(|row| row.coordinate == coordinate && row.function.body.is_some())
            .ok_or_else(source_raw_physical_error_v29)?;
        let definitions = inventory
            .definitions()
            .get(function.definitions.clone())
            .ok_or_else(source_raw_physical_error_v29)?;
        let source_blocks = inventory
            .blocks()
            .get(function.blocks.clone())
            .ok_or_else(source_raw_physical_error_v29)?;
        let mut scratch = 0;
        let mut values = call_splice_vec_v1(definitions.len(), budget, &mut scratch)
            .map_err(source_address_call_error_v29)?;
        let mut blocks = call_splice_vec_v1(source_blocks.len(), budget, &mut scratch)
            .map_err(source_address_call_error_v29)?;
        for definition in definitions {
            budget.charge_work(1)?;
            values.push((
                definition.value.ok_or_else(source_raw_physical_error_v29)?,
                definition.ty,
            ));
        }
        for block in source_blocks {
            budget.charge_work(1)?;
            if block.coordinate.function != coordinate {
                return Err(source_raw_physical_error_v29());
            }
            blocks.push(block.block.id);
        }
        call_splice_sort_work_v1(values.len(), budget).map_err(source_address_call_error_v29)?;
        values.sort_unstable_by_key(|row| row.0);
        call_splice_sort_work_v1(blocks.len(), budget).map_err(source_address_call_error_v29)?;
        blocks.sort_unstable();
        budget.charge_work(argument_sum_v1(&[values.len(), blocks.len()])?)?;
        if values.windows(2).any(|pair| pair[0].0 == pair[1].0)
            || blocks.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(source_raw_physical_error_v29());
        }
        let index = CallSpliceIndexV1 { blocks, values };
        call_splice_check_body_v1(function.function, &index, false, budget)
            .map_err(source_address_call_error_v29)?;
        Self::prepare_indexed(
            function.function,
            slots,
            accesses,
            index,
            &inventory.owner().module().storage_layouts,
            compiler,
            budget,
        )
    }

    fn prepare_indexed(
        function: &'kir Function,
        slots: &[ScopedSourceSlotV29],
        accesses: &[SourceAddressAccessV29],
        index: CallSpliceIndexV1<'kir>,
        layouts: &[fe2o3_kernel_ir::StorageLayoutV1],
        compiler: Option<&scoped_raw_admission_v29::CheckedCompilerEnumMemoryV55<'_>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        if compiler.is_some_and(|checked| !checked.belongs_to(function)) {
            return Err(scoped_compiler_enum_error_v55());
        }
        budget.reserve_storage(source_tag_geometry_headers_v43()?)?;
        let body = function
            .body
            .as_ref()
            .ok_or_else(source_raw_physical_error_v29)?;
        let entry = body
            .blocks
            .first()
            .ok_or_else(source_raw_physical_error_v29)?
            .id;
        let mut blocks = emission_vec_v1(body.blocks.len(), budget)?;
        budget.charge_work(body.blocks.len())?;
        blocks.extend(body.blocks.iter().map(|block| (block.id, block)));
        call_splice_sort_work_v1(blocks.len(), budget).map_err(source_address_call_error_v29)?;
        blocks.sort_unstable_by_key(|(id, _)| *id);
        let mut pointer_cells = emission_vec_v1(slots.len(), budget)?;
        let mut object_schemas = emission_vec_v1(slots.len(), budget)?;
        let mut pointer_cell_count = 0;
        for slot in slots {
            budget.charge_work(5)?;
            let scalar = match (
                slot.origin.identity,
                slot.origin.source,
                slot.representation,
            ) {
                (
                    ScopedAllocationIdentityV29::LegacyLocal(_),
                    ScopedAllocationSourceV29::Legacy
                    | ScopedAllocationSourceV29::OriginalArray { .. },
                    ScopedSlotRepresentationV29::ScalarArray(_),
                ) => slot.scalar_array()?,
                (
                    ScopedAllocationIdentityV29::OriginalObject { .. },
                    ScopedAllocationSourceV29::OriginalObject {
                        schema: original, ..
                    },
                    ScopedSlotRepresentationV29::Object {
                        schema,
                        bytes,
                        alignment,
                    },
                ) if original == schema => {
                    let layout = layouts
                        .get(schema.0 as usize)
                        .ok_or_else(scoped_object_pending_v29)?;
                    if layout.size != bytes || layout.alignment != alignment {
                        return Err(source_raw_physical_error_v29());
                    }
                    let cell = if matches!(
                        layout.kind,
                        fe2o3_kernel_ir::StorageLayoutKindV1::Pointer(_)
                    ) {
                        let cell = pointer_cell_count;
                        pointer_cell_count = argument_sum_v1(&[pointer_cell_count, 1])?;
                        Some(cell)
                    } else {
                        None
                    };
                    object_schemas.push(Some(schema));
                    pointer_cells.push(cell);
                    continue;
                }
                _ => return Err(source_raw_physical_error_v29()),
            };
            object_schemas.push(None);
            let cell = match scalar.element.element {
                PrivateRetainedElementFactsV1::Scalar(_) => None,
                PrivateRetainedElementFactsV1::ThinPointer { .. } => {
                    if scalar.length != 1 {
                        return Err(source_raw_physical_error_v29());
                    }
                    let index = pointer_cell_count;
                    pointer_cell_count = argument_sum_v1(&[pointer_cell_count, 1])?;
                    Some(index)
                }
            };
            pointer_cells.push(cell);
        }
        let nodes = argument_sum_v1(&[
            index.values.len(),
            argument_product_v1(argument_product_v1(blocks.len(), 2)?, pointer_cell_count)?,
            1,
        ])?;
        let mut origins = emission_vec_v1(nodes, budget)?;
        budget.charge_work(nodes)?;
        origins.resize(nodes, SourceAddressOriginV29::Pending);
        let mut zero_offsets = emission_vec_v1(index.values.len(), budget)?;
        budget.charge_work(index.values.len())?;
        zero_offsets.resize(index.values.len(), false);
        let mut graph = Self {
            index,
            blocks,
            pointer_cells,
            pointer_cell_count,
            pointer_subcells: Vec::new(),
            pointer_subcell_addresses: Vec::new(),
            object_layouts: source_static_object_layouts_v29(layouts, budget)?,
            object_schemas,
            zero_offsets,
            origins,
            projections: Vec::new(),
            locations: Vec::new(),
            compiler_references: match compiler {
                Some(checked) => checked.reference_uses(budget)?,
                None => Vec::new(),
            },
        };
        for reference in &graph.compiler_references {
            budget.charge_work(6)?;
            let slot = slots
                .get(reference.custody.slot)
                .ok_or_else(source_enum_tag_error_v55)?;
            let backing = match (reference.custody.backing, slot.representation) {
                (
                    SourceCompilerEnumReferenceBackingV55::Scalar,
                    ScopedSlotRepresentationV29::ScalarArray(_),
                ) => true,
                (
                    SourceCompilerEnumReferenceBackingV55::Object(expected),
                    ScopedSlotRepresentationV29::Object { schema, .. },
                ) => expected == schema,
                _ => false,
            };
            if !backing
                || slot.instance != reference.custody.loan.origin_instance
                || slot.origin.semantic_type != reference.custody.loan.origin_type
            {
                return Err(source_enum_tag_error_v55());
            }
        }
        graph.projections = source_static_object_projections_v29(&graph, layouts, budget)?;
        graph.validate_coordinates(slots, accesses, &[], budget)?;
        for (index, (_, ty)) in graph.index.values.iter().enumerate() {
            budget.charge_work(1)?;
            graph.origins[index] = match ty {
                Type::Pointer(pointer)
                    if matches!(
                        pointer.address_space,
                        AddressSpace::Private | AddressSpace::Generic
                    ) =>
                {
                    SourceAddressOriginV29::Unknown
                }
                _ => SourceAddressOriginV29::Exact(None),
            };
        }
        for (block_index, (_, block)) in graph.blocks.iter().enumerate() {
            budget.charge_work(argument_sum_v1(&[
                1,
                block.parameters.len(),
                block.operations.len(),
            ])?)?;
            for parameter in &block.parameters {
                if block.id != entry && matches!(parameter.ty, Type::Pointer(_)) {
                    let index = graph.value(parameter.id, budget)?;
                    graph.origins[index] = SourceAddressOriginV29::Pending;
                }
            }
            for (ordinal, operation) in block.operations.iter().enumerate() {
                if matches!(operation.kind, OperationKind::Constant(Constant::Index(0))) {
                    let [result] = operation.results.as_slice() else {
                        return Err(source_raw_physical_error_v29());
                    };
                    Self::same_type(&result.ty, &Type::INDEX, budget)?;
                    let index = graph.value(result.id, budget)?;
                    graph.zero_offsets[index] = true;
                }
                if let [result] = operation.results.as_slice()
                    && matches!(result.ty, Type::Pointer(_))
                    && (matches!(
                        operation.kind,
                        OperationKind::Cast {
                            kind: CastKind::RestrictPointerAccess | CastKind::PointerToGeneric,
                            ..
                        } | OperationKind::Select { .. }
                            | OperationKind::GetElementPointer { .. }
                            | OperationKind::Storage(ScopedObjectOperationV29::Project { .. })
                    ) || (matches!(
                        operation.kind,
                        OperationKind::Load { .. }
                            | OperationKind::Storage(ScopedObjectOperationV29::ReadValue { .. })
                    ) && Self::access(accesses, block.id, ordinal, budget)?.is_some()))
                {
                    let index = graph.value(result.id, budget)?;
                    graph.origins[index] = SourceAddressOriginV29::Pending;
                }
            }
            if block.id == entry {
                for cell in 0..graph.pointer_cell_count {
                    budget.charge_work(1)?;
                    let node = graph.memory(block_index, cell, false)?;
                    graph.origins[node] = SourceAddressOriginV29::Unknown;
                }
            }
        }
        // Scan definitions once, then rejoin each seed by its actual Alloca.
        // The input roster and a supplied ValueId alone cannot create a seed.
        let mut allocations = emission_vec_v1(slots.len(), budget)?;
        for (_, block) in &graph.blocks {
            budget.charge_work(argument_sum_v1(&[1, block.operations.len()])?)?;
            for operation in &block.operations {
                if matches!(operation.kind, OperationKind::Alloca { .. }) {
                    let [result] = operation.results.as_slice() else {
                        return Err(source_raw_physical_error_v29());
                    };
                    if let Some(compiler) = compiler
                        && compiler.contains_allocation(result.id, budget)?
                    {
                        // Only the closed compiler-only use census establishes
                        // disjointness. These are never original source seeds.
                        let node = graph.value(result.id, budget)?;
                        if graph.origins[node] != SourceAddressOriginV29::Unknown {
                            return Err(scoped_compiler_enum_error_v55());
                        }
                        graph.origins[node] = SourceAddressOriginV29::Exact(None);
                        continue;
                    }
                    if allocations.len() == slots.len() {
                        return Err(source_raw_physical_error_v29());
                    }
                    allocations.push((result.id, operation));
                }
            }
        }
        if allocations.len() != slots.len() {
            return Err(source_raw_physical_error_v29());
        }
        call_splice_sort_work_v1(allocations.len(), budget)
            .map_err(source_address_call_error_v29)?;
        allocations.sort_unstable_by_key(|(id, _)| *id);
        for (slot_index, slot) in slots.iter().enumerate() {
            budget.charge_work(call_splice_search_work_v1(allocations.len()))?;
            let allocation = allocations
                .binary_search_by_key(&slot.origin.pointer, |(id, _)| *id)
                .map_err(|_| source_raw_physical_error_v29())?;
            budget.charge_work(1)?;
            match slot.representation {
                ScopedSlotRepresentationV29::ScalarArray(scalar) => {
                    slot.scalar_array()?;
                    private_retained_check_allocation_operation_v1(
                        allocations[allocation].1,
                        slot.origin.pointer,
                        scalar.count.map(|row| row.0),
                        scalar.element,
                        budget,
                    )
                    .map_err(scoped_slot_relation_error_v29)?;
                }
                ScopedSlotRepresentationV29::Object {
                    schema, alignment, ..
                } => {
                    check_scoped_object_alloca_v29(
                        allocations[allocation].1,
                        slot.origin.pointer,
                        schema,
                        alignment,
                        budget,
                    )?;
                }
            }
            let node = graph.value(slot.origin.pointer, budget)?;
            if graph.origins[node] != SourceAddressOriginV29::Unknown {
                return Err(source_raw_physical_error_v29());
            }
            graph.origins[node] = SourceAddressOriginV29::Exact(Some(slot_index));
        }
        let allocation_bytes = argument_product_v1(
            allocations.capacity(),
            std::mem::size_of::<(ValueId, &Operation)>(),
        )?;
        drop(allocations);
        budget.release_storage(allocation_bytes)?;
        let unknown = graph.unknown();
        graph.origins[unknown] = SourceAddressOriginV29::Unknown;
        graph.prepare_pointer_subcells(entry, slots, accesses, budget)?;
        Ok(graph)
    }

    fn solve(
        self,
        slots: &[ScopedSourceSlotV29],
        accesses: &[SourceAddressAccessV29],
        kills: &[SourceAddressKillV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        self.solve_geometry(
            slots,
            accesses,
            kills,
            SourceAddressGeometryV29::Scalar,
            budget,
        )
    }

    fn solve_pending_indices(
        self,
        slots: &[ScopedSourceSlotV29],
        accesses: &[SourceAddressAccessV29],
        kills: &[SourceAddressKillV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<PendingSourceIndexPhysicalV29<'kir>, ProductionSemanticKirErrorV1> {
        let graph = self.solve_geometry(
            slots,
            accesses,
            kills,
            SourceAddressGeometryV29::PendingIndices,
            budget,
        )?;
        Ok(PendingSourceIndexPhysicalV29 { graph })
    }

    fn solve_geometry(
        mut self,
        slots: &[ScopedSourceSlotV29],
        accesses: &[SourceAddressAccessV29],
        kills: &[SourceAddressKillV29],
        geometry: SourceAddressGeometryV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        self.validate_coordinates(slots, accesses, kills, budget)?;
        if slots.is_empty() {
            // prepare_indexed checked the complete actual Alloca census. With
            // no local objects, no value can alias a member of this domain.
            // This says nothing about external memory bounds or validity.
            if !accesses.is_empty() || !kills.is_empty() || self.pointer_cell_count != 0 {
                return Err(source_raw_physical_error_v29());
            }
            budget.charge_work(self.origins.len())?;
            self.origins.fill(SourceAddressOriginV29::Exact(None));
            self.validate_solution(slots, accesses, geometry, budget)?;
            self.validate_uses(accesses, budget)?;
            return Ok(self);
        }
        let mut links = 0;
        self.dependencies(accesses, kills, budget, |_, _, _, _| {
            links = argument_sum_v1(&[links, 1])?;
            Ok(())
        })?;
        let mut work = origin_worklist_v1::OriginWorkV1::new(self.origins.len(), links, budget)
            .map_err(source_address_equation_error_v29)?;
        let mut locations = if !self.projections.is_empty() {
            budget.reserve_storage(std::mem::size_of::<
                Option<
                    origin_worklist_v1::OriginWorkV1<
                        Option<SourceStaticObjectLocationV29>,
                        SourceStaticObjectTransferV29,
                    >,
                >,
            >())?;
            Some(
                origin_worklist_v1::OriginWorkV1::<
                    Option<SourceStaticObjectLocationV29>,
                    SourceStaticObjectTransferV29,
                >::new(self.origins.len(), links, budget)
                .map_err(source_address_equation_error_v29)?,
            )
        } else {
            None
        };
        for &seed in &self.origins {
            work.seed_next(seed, budget)
                .map_err(source_address_equation_error_v29)?;
            if let Some(locations) = &mut locations {
                let seed = match seed {
                    SourceAddressOriginV29::Exact(Some(slot)) => {
                        let schema = match slots
                            .get(slot)
                            .ok_or_else(source_raw_physical_error_v29)?
                            .representation
                        {
                            ScopedSlotRepresentationV29::Object { schema, .. } => Some(schema),
                            ScopedSlotRepresentationV29::ScalarArray(_) => None,
                        };
                        origin_worklist_v1::OriginStateV1::Exact(Some(
                            SourceStaticObjectLocationV29 {
                                slot,
                                offset: 0,
                                schema,
                            },
                        ))
                    }
                    SourceAddressOriginV29::Exact(None) => {
                        origin_worklist_v1::OriginStateV1::Exact(None)
                    }
                    SourceAddressOriginV29::Pending => origin_worklist_v1::OriginStateV1::Pending,
                    SourceAddressOriginV29::Unknown => origin_worklist_v1::OriginStateV1::Unknown,
                };
                locations
                    .seed_next(seed, budget)
                    .map_err(source_address_equation_error_v29)?;
            }
        }
        self.dependencies(
            accesses,
            kills,
            budget,
            |source, target, transfer, budget| {
                work.add_link(source, target, budget)
                    .map_err(source_address_equation_error_v29)?;
                if let Some(locations) = &mut locations {
                    locations
                        .add_transfer(source, target, transfer, budget)
                        .map_err(source_address_equation_error_v29)?;
                }
                Ok(())
            },
        )?;
        self.origins = work
            .solve(budget)
            .map_err(source_address_equation_error_v29)?;
        if let Some(locations) = locations {
            self.locations = locations
                .solve_with(budget, |transfer, value, budget| {
                    source_static_object_apply_v29(transfer, value, slots, budget)
                })
                .map_err(source_address_equation_error_v29)?;
        }
        self.validate_solution(slots, accesses, geometry, budget)?;
        self.validate_uses(accesses, budget)?;
        Ok(self)
    }

    fn validate_coordinates(
        &self,
        slots: &[ScopedSourceSlotV29],
        accesses: &[SourceAddressAccessV29],
        kills: &[SourceAddressKillV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut prior = None;
        for row in accesses {
            budget.charge_work(3)?;
            let key = (row.block, row.operation, row.footprint);
            if prior.is_some_and(|prior| prior >= key) || row.slot >= slots.len() {
                return Err(source_raw_physical_error_v29());
            }
            prior = Some(key);
            let block = self.blocks[self.block(row.block, budget)?].1;
            let operation = block
                .operations
                .get(row.operation)
                .ok_or_else(source_raw_physical_error_v29)?;
            if source_address_footprint_v33(operation, row.footprint, budget)?.is_none() {
                return Err(source_raw_physical_error_v29());
            }
        }
        if !kills.is_empty() {
            with_canonical_call_scratch_v1(budget, |budget| {
                budget.reserve_storage(source_reference_emission_headers_v29::<
                    SourceAddressKillOrderFrameV45<'_>,
                >()?)?;
                let mut prior = None;
                for row in kills {
                    budget.charge_work(8)?;
                    let key = (row.block, row.gap, row.source_order, row.slot);
                    if prior.is_some_and(|prior| prior >= key) || row.slot >= slots.len() {
                        return Err(source_raw_physical_error_v29());
                    }
                    prior = Some(key);
                    if row.gap
                        > self.blocks[self.block(row.block, budget)?]
                            .1
                            .operations
                            .len()
                    {
                        return Err(source_raw_physical_error_v29());
                    }
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    fn dependencies(
        &self,
        accesses: &[SourceAddressAccessV29],
        kills: &[SourceAddressKillV29],
        budget: &mut ArgumentBudgetV1<'_>,
        mut visit: impl FnMut(
            usize,
            usize,
            SourceStaticObjectTransferV29,
            &mut ArgumentBudgetV1<'_>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut current = emission_vec_v1(self.pointer_cell_count, budget)?;
        let owned = argument_product_v1(current.capacity(), std::mem::size_of::<usize>())?;
        budget.charge_work(self.pointer_cell_count)?;
        current.resize(self.pointer_cell_count, self.unknown());
        let mut kill = 0;
        for (block_index, (_, block)) in self.blocks.iter().enumerate() {
            budget.charge_work(argument_sum_v1(&[1, current.len()])?)?;
            for (cell, value) in current.iter_mut().enumerate() {
                *value = self.memory(block_index, cell, false)?;
            }
            for gap in 0..=block.operations.len() {
                budget.charge_work(1)?;
                while let Some(row) = kills.get(kill)
                    && (row.block, row.gap) == (block.id, gap)
                {
                    budget.charge_work(2)?;
                    for cell in self.pointer_cells[row.slot]
                        .into_iter()
                        .chain(self.pointer_cell_range(row.slot, budget)?)
                    {
                        current[cell] = self.unknown();
                    }
                    kill += 1;
                }
                let Some(operation) = block.operations.get(gap) else {
                    continue;
                };
                match &operation.kind {
                    OperationKind::Storage(ScopedObjectOperationV29::Project { base, .. }) => {
                        let [result] = operation.results.as_slice() else {
                            return Err(source_raw_physical_error_v29());
                        };
                        let node = self.value(result.id, budget)?;
                        budget.charge_work(call_splice_search_work_v1(self.projections.len()))?;
                        let transfer = self
                            .projections
                            .binary_search_by_key(&node, |row| row.0)
                            .ok()
                            .map(|index| self.projections[index].1)
                            .ok_or_else(source_raw_physical_error_v29)?;
                        visit(self.value(*base, budget)?, node, transfer, budget)?;
                    }
                    OperationKind::GetElementPointer { base, .. } => {
                        // Solve origins before applying private geometry. An
                        // external descriptor may have a nonliteral offset.
                        self.check_gep_type(operation, budget)?;
                        visit(
                            self.value(*base, budget)?,
                            self.value(operation.results[0].id, budget)?,
                            SourceStaticObjectTransferV29::Identity,
                            budget,
                        )?;
                    }
                    OperationKind::Cast {
                        kind: CastKind::RestrictPointerAccess | CastKind::PointerToGeneric,
                        value,
                        ..
                    } => {
                        let [result] = operation.results.as_slice() else {
                            return Err(source_raw_physical_error_v29());
                        };
                        self.check_cast(operation, budget)?;
                        visit(
                            self.value(*value, budget)?,
                            self.value(result.id, budget)?,
                            SourceStaticObjectTransferV29::Identity,
                            budget,
                        )?;
                    }
                    OperationKind::Select {
                        true_value,
                        false_value,
                        ..
                    } if operation
                        .results
                        .first()
                        .is_some_and(|result| matches!(result.ty, Type::Pointer(_))) =>
                    {
                        let [result] = operation.results.as_slice() else {
                            return Err(source_raw_physical_error_v29());
                        };
                        Self::same_type(self.ty(*true_value, budget)?, &result.ty, budget)?;
                        Self::same_type(self.ty(*false_value, budget)?, &result.ty, budget)?;
                        visit(
                            self.value(*true_value, budget)?,
                            self.value(result.id, budget)?,
                            SourceStaticObjectTransferV29::Identity,
                            budget,
                        )?;
                        visit(
                            self.value(*false_value, budget)?,
                            self.value(result.id, budget)?,
                            SourceStaticObjectTransferV29::Identity,
                            budget,
                        )?;
                    }
                    OperationKind::Load { .. }
                    | OperationKind::Store { .. }
                    | OperationKind::Storage(
                        ScopedObjectOperationV29::ReadValue { .. }
                        | ScopedObjectOperationV29::WriteValue { .. },
                    ) => {
                        if let Some(row) = Self::access(accesses, block.id, gap, budget)?
                            && let Some(cell) =
                                self.access_pointer_cell(Some(row), operation, budget)?
                        {
                            match &operation.kind {
                                OperationKind::Load { .. }
                                | OperationKind::Storage(ScopedObjectOperationV29::ReadValue {
                                    ..
                                }) => {
                                    let [result] = operation.results.as_slice() else {
                                        return Err(source_raw_physical_error_v29());
                                    };
                                    if !matches!(result.ty, Type::Pointer(_)) {
                                        return Err(source_raw_physical_error_v29());
                                    }
                                    visit(
                                        current[cell],
                                        self.value(result.id, budget)?,
                                        SourceStaticObjectTransferV29::Identity,
                                        budget,
                                    )?;
                                }
                                OperationKind::Store { value, .. }
                                | OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                                    value,
                                    ..
                                }) => {
                                    if !matches!(self.ty(*value, budget)?, Type::Pointer(_)) {
                                        return Err(source_raw_physical_error_v29());
                                    }
                                    current[cell] = self.value(*value, budget)?;
                                }
                                _ => unreachable!(),
                            }
                        }
                    }
                    OperationKind::Storage(ScopedObjectOperationV29::SetDiscriminant {
                        ..
                    }) => {
                        let tag = source_address_tag_access_v43(operation)?
                            .ok_or_else(source_raw_physical_error_v29)?;
                        let row = Self::access(accesses, block.id, gap, budget)?
                            .ok_or_else(source_raw_physical_error_v29)?;
                        self.visit_tag_overwritten_pointer_cells_v43(
                            tag,
                            row.slot,
                            budget,
                            |cell, _| {
                                current[cell] = self.unknown();
                                Ok(())
                            },
                        )?;
                    }
                    _ => {}
                }
            }
            for (cell, &value) in current.iter().enumerate() {
                visit(
                    value,
                    self.memory(block_index, cell, true)?,
                    SourceStaticObjectTransferV29::Identity,
                    budget,
                )?;
            }
            block
                .terminator
                .as_ref()
                .ok_or_else(source_raw_physical_error_v29)?
                .try_visit_edges_v1(|target, arguments| {
                    let target_index = self.block(target, budget)?;
                    let target = self.blocks[target_index].1;
                    budget.charge_work(argument_sum_v1(&[
                        arguments.len(),
                        self.pointer_cell_count,
                    ])?)?;
                    if arguments.len() != target.parameters.len() {
                        return Err(source_raw_physical_error_v29());
                    }
                    for (&argument, parameter) in arguments.iter().zip(&target.parameters) {
                        Self::same_type(self.ty(argument, budget)?, &parameter.ty, budget)?;
                        if matches!(parameter.ty, Type::Pointer(_)) {
                            visit(
                                self.value(argument, budget)?,
                                self.value(parameter.id, budget)?,
                                SourceStaticObjectTransferV29::Identity,
                                budget,
                            )?;
                        }
                    }
                    for cell in 0..self.pointer_cell_count {
                        visit(
                            self.memory(block_index, cell, true)?,
                            self.memory(target_index, cell, false)?,
                            SourceStaticObjectTransferV29::Identity,
                            budget,
                        )?;
                    }
                    Ok(())
                })?;
        }
        if kill != kills.len() {
            return Err(source_raw_physical_error_v29());
        }
        drop(current);
        budget.release_storage(owned)?;
        Ok(())
    }

    fn check_zero_gep(
        &self,
        operation: &Operation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_gep_type(operation, budget)?;
        let OperationKind::GetElementPointer { offset, .. } = operation.kind else {
            return Err(source_raw_physical_error_v29());
        };
        if !self.zero_offsets[self.value(offset, budget)?] {
            return Err(source_raw_physical_error_v29());
        }
        Ok(())
    }

    fn check_gep_type(
        &self,
        operation: &Operation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let OperationKind::GetElementPointer { base, offset } = &operation.kind else {
            return Err(source_raw_physical_error_v29());
        };
        let [result] = operation.results.as_slice() else {
            return Err(source_raw_physical_error_v29());
        };
        budget.charge_work(2)?;
        if !matches!(result.ty, Type::Pointer(_)) {
            return Err(source_raw_physical_error_v29());
        }
        Self::same_type(self.ty(*base, budget)?, &result.ty, budget)?;
        Self::same_type(self.ty(*offset, budget)?, &Type::INDEX, budget)
    }

    fn check_cast(
        &self,
        operation: &Operation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let OperationKind::Cast { kind, value, to } = &operation.kind else {
            return Err(source_raw_physical_error_v29());
        };
        let [result] = operation.results.as_slice() else {
            return Err(source_raw_physical_error_v29());
        };
        let (Type::Pointer(source), Type::Pointer(target)) = (self.ty(*value, budget)?, &result.ty)
        else {
            return Err(source_raw_physical_error_v29());
        };
        Self::same_type(to, &result.ty, budget)?;
        Self::same_type(&source.pointee, &target.pointee, budget)?;
        let valid = match kind {
            CastKind::RestrictPointerAccess => {
                source.address_space == target.address_space
                    && source.access == AccessMode::ReadWrite
                    && target.access == AccessMode::ReadOnly
            }
            CastKind::PointerToGeneric => {
                target.address_space == AddressSpace::Generic
                    && matches!(
                        source.address_space,
                        AddressSpace::Private
                            | AddressSpace::Global
                            | AddressSpace::Workgroup
                            | AddressSpace::Constant
                    )
                    && source.access == target.access
                    && (source.address_space != AddressSpace::Constant
                        || source.access == AccessMode::ReadOnly)
            }
            _ => false,
        };
        if !valid {
            return Err(source_raw_physical_error_v29());
        }
        Ok(())
    }

    fn validate_solution(
        &self,
        slots: &[ScopedSourceSlotV29],
        accesses: &[SourceAddressAccessV29],
        geometry: SourceAddressGeometryV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut claimed = 0;
        for (_, block) in &self.blocks {
            budget.charge_work(argument_sum_v1(&[1, block.operations.len()])?)?;
            for (ordinal, operation) in block.operations.iter().enumerate() {
                if let OperationKind::GetElementPointer { base, .. } = operation.kind {
                    // Includes unused GEPs. Unknown and mixed origins refuse;
                    // no descriptor receipt can waive private scalar geometry.
                    match (geometry, self.exact(base, budget)?) {
                        (SourceAddressGeometryV29::Scalar, Some(_)) => {
                            self.check_zero_gep(operation, budget)?;
                        }
                        _ => self.check_gep_type(operation, budget)?,
                    }
                }
                if let Some(tag) = source_address_tag_access_v43(operation)? {
                    let row = Self::access(accesses, block.id, ordinal, budget)?
                        .ok_or_else(source_raw_physical_error_v29)?;
                    if self.exact(tag.pointer, budget)? != Some(row.slot) {
                        return Err(source_raw_physical_error_v29());
                    }
                    let location = self.object_location(tag.pointer, budget)?;
                    if location.slot != row.slot
                        || location.offset != 0
                        || location.schema != self.object_schemas.get(row.slot).copied().flatten()
                    {
                        return Err(scoped_object_pending_v29());
                    }
                    let slot = slots
                        .get(row.slot)
                        .ok_or_else(source_raw_physical_error_v29)?;
                    if self.object_tag_root_range_v43(tag, row.slot, budget)?
                        != self.object_tag_range_v43(tag, slot, budget)?
                    {
                        return Err(source_raw_physical_error_v29());
                    }
                    claimed = argument_sum_v1(&[claimed, 1])?;
                    continue;
                }
                let Some(SourceAddressValueAccessV29 {
                    pointer,
                    access,
                    value,
                    writing,
                    object,
                }) = source_address_value_access_v29(operation)?
                else {
                    continue;
                };
                let Type::Pointer(ty) = self.ty(pointer, budget)? else {
                    return Err(source_raw_physical_error_v29());
                };
                if ty.address_space != access.address_space
                    || !access.alignment.is_power_of_two()
                    || (writing
                        && ty.access != AccessMode::ReadWrite
                        && !(object && ty.access == AccessMode::WriteOnly))
                    || (!writing && ty.access == AccessMode::WriteOnly)
                {
                    return Err(source_raw_physical_error_v29());
                }
                if !object {
                    Self::same_type(&ty.pointee, self.ty(value, budget)?, budget)?;
                }
                match Self::access(accesses, block.id, ordinal, budget)? {
                    Some(row) => {
                        if self.exact(pointer, budget)? != Some(row.slot) {
                            return Err(source_raw_physical_error_v29());
                        }
                        let slot = slots
                            .get(row.slot)
                            .ok_or_else(source_raw_physical_error_v29)?;
                        match slot.representation {
                            ScopedSlotRepresentationV29::ScalarArray(scalar) => {
                                if object
                                    || (matches!(geometry, SourceAddressGeometryV29::Scalar)
                                        && scalar.length != 1)
                                    || scalar.bytes
                                        != scalar
                                            .length
                                            .checked_mul(scalar.element.size)
                                            .ok_or(ArgumentResourceV1::Arithmetic)?
                                    || access.alignment > scalar.element.alignment
                                    || !scalar
                                        .element
                                        .element
                                        .matches_borrowed(&ty.pointee, budget)?
                                {
                                    return Err(source_raw_physical_error_v29());
                                }
                            }
                            ScopedSlotRepresentationV29::Object { .. } => {
                                if !object
                                    || ty.address_space != AddressSpace::Private
                                    || self.object_location(pointer, budget)?.slot != row.slot
                                {
                                    return Err(source_raw_physical_error_v29());
                                }
                                self.object_value_range(
                                    pointer,
                                    slot,
                                    self.ty(value, budget)?,
                                    access,
                                    budget,
                                )?;
                            }
                        }
                        if !writing && self.pointer_cells[row.slot].is_some() {
                            // An unwritten pointer read is invalid even when
                            // the resulting SSA value is subsequently unused.
                            self.exact(value, budget)?;
                        }
                        // Observed subcells may transport opaque raw pointer
                        // bits without pointee authority. Their complete byte
                        // initialization history is checked independently, and
                        // validate_uses rejects every opaque address/escape use.
                        claimed = argument_sum_v1(&[claimed, 1])?;
                    }
                    None => {
                        // Unknown Private/Generic pointers may alias every
                        // tracked cell. They cannot be ignored as unrelated.
                        if object || self.exact(pointer, budget)?.is_some() {
                            return Err(source_raw_physical_error_v29());
                        }
                    }
                }
            }
        }
        if claimed != accesses.len() {
            return Err(source_raw_physical_error_v29());
        }
        Ok(())
    }

    fn validate_uses(
        &self,
        accesses: &[SourceAddressAccessV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let subcell_base = self
            .pointer_cell_count
            .checked_sub(self.pointer_subcells.len())
            .ok_or(ArgumentResourceV1::Accounting)?;
        for (_, block) in &self.blocks {
            budget.charge_work(argument_sum_v1(&[1, block.operations.len()])?)?;
            for (position, operation) in block.operations.iter().enumerate() {
                let access = Self::access(accesses, block.id, position, budget)?;
                let compiler_reference =
                    self.compiler_reference_use_v55(block.id, position, operation, budget)?;
                let mut ordinal = 0;
                operation.kind.try_visit_operands(|value| {
                    budget.charge_work(1)?;
                    let component = ordinal;
                    ordinal = argument_sum_v1(&[ordinal, 1])?;
                    match self.origins[self.value(value, budget)?] {
                        SourceAddressOriginV29::Exact(None) => return Ok(()),
                        SourceAddressOriginV29::Exact(Some(_)) => {}
                        SourceAddressOriginV29::Unknown
                            if component == 1
                                && matches!(
                                    operation.kind,
                                    OperationKind::Storage(
                                        ScopedObjectOperationV29::WriteValue { .. }
                                    )
                                )
                                && matches!(self.ty(value, budget)?, Type::Pointer(_))
                                && self
                                    .access_pointer_cell(access, operation, budget)?
                                    .is_some_and(|cell| cell >= subcell_base) =>
                        {
                            return Ok(());
                        }
                        _ => return Err(source_raw_physical_error_v29()),
                    }
                    let allowed = match operation.kind {
                        OperationKind::Cast {
                            kind: CastKind::RestrictPointerAccess | CastKind::PointerToGeneric,
                            ..
                        }
                        | OperationKind::Load { .. }
                        | OperationKind::Storage(ScopedObjectOperationV29::ReadValue { .. })
                        | OperationKind::Storage(
                            ScopedObjectOperationV29::ReadDiscriminant { .. }
                            | ScopedObjectOperationV29::SetDiscriminant { .. },
                        )
                        | OperationKind::Storage(ScopedObjectOperationV29::Project { .. })
                        | OperationKind::GetElementPointer { .. } => component == 0,
                        OperationKind::Select { .. } => component == 1 || component == 2,
                        OperationKind::Store { .. }
                        | OperationKind::Storage(ScopedObjectOperationV29::WriteValue { .. }) => {
                            component == 0
                                || (component == 1
                                    && (compiler_reference.is_some_and(|row| row.value == value)
                                        || self
                                            .access_pointer_cell(access, operation, budget)?
                                            .is_some()))
                        }
                        _ => false,
                    };
                    if !allowed {
                        return Err(source_raw_physical_error_v29());
                    }
                    Ok(())
                })?;
            }
            let terminator = block
                .terminator
                .as_ref()
                .ok_or_else(source_raw_physical_error_v29)?;
            // Expanded child returns are actual edges. A surviving Return is
            // an external escape, never an implied child-to-caller handoff.
            match terminator {
                Terminator::Return { values } => {
                    budget.charge_work(values.len())?;
                    for &value in values {
                        if self.exact(value, budget)?.is_some() {
                            return Err(source_raw_physical_error_v29());
                        }
                    }
                }
                Terminator::ConditionalBranch { condition, .. } => {
                    Self::same_type(self.ty(*condition, budget)?, &Type::BOOL, budget)?;
                }
                Terminator::Switch { selector, .. }
                | Terminator::IntegerSwitch { selector, .. } => {
                    if self.exact(*selector, budget)?.is_some() {
                        return Err(source_raw_physical_error_v29());
                    }
                }
                Terminator::Branch { .. } | Terminator::Unreachable => {}
            }
            terminator.try_visit_edges_v1(|target, arguments| {
                let target = self.blocks[self.block(target, budget)?].1;
                budget.charge_work(arguments.len())?;
                for (&argument, parameter) in arguments.iter().zip(&target.parameters) {
                    if matches!(parameter.ty, Type::Pointer(_)) {
                        let from = self.origins[self.value(argument, budget)?];
                        let to = self.origins[self.value(parameter.id, budget)?];
                        match (from, to) {
                            (
                                SourceAddressOriginV29::Exact(left),
                                SourceAddressOriginV29::Exact(right),
                            ) if left == right => {}
                            // In-root typed transport only. The complete use
                            // census still rejects opaque address and Return uses.
                            (SourceAddressOriginV29::Unknown, SourceAddressOriginV29::Unknown)
                                if !self.pointer_subcells.is_empty() => {}
                            _ => return Err(source_raw_physical_error_v29()),
                        }
                    }
                }
                Ok(())
            })?;
        }
        Ok(())
    }
}
