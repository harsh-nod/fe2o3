#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecutionIdentitySourceKindV1 {
    Entry(usize),
    RetainedEntry(usize),
    Event(usize),
    Edge(usize),
    Phi(SsaBlockIdV1),
    Header(SsaBlockIdV1),
}

struct ExecutionIdentitySourceRowV1 {
    instance: ProductionCallInstanceIdV1,
    value: Option<SsaValueV1>,
    local: SemanticLocalIdV1,
    selection: ExecutionIdentitySelectionV1,
    kind: ExecutionIdentitySourceKindV1,
}

struct ExecutionIdentitySourceIndexV1<'a, 'source> {
    instances: &'a ExecutionInstancesV29<'source>,
    rows: Vec<ExecutionIdentitySourceRowV1>,
    values: BTreeMap<(usize, SsaValueV1), usize>,
    retained: BTreeMap<(usize, u32), usize>,
    headers: BTreeMap<(usize, u32, u32), usize>,
    tails: BTreeMap<(usize, u32, u32), Option<SsaValueV1>>,
    events: Vec<Vec<UnitLocalSourceIndexV1>>,
    incoming: BTreeMap<(usize, u32), Vec<usize>>,
    calls: BTreeMap<(usize, u32), usize>,
    channels: usize,
}

fn execution_identity_block_active_v1(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    block: SsaBlockIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    instances
        .block_reachable(instance, SemanticBlockIdV1::from_index(block.get()))
        .ok_or_else(execution_identity_error_v1)
}

fn execution_identity_successor_active_v1(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    block: SsaBlockIdV1,
    role: SemanticEdgeRoleV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    if !execution_identity_block_active_v1(instances, instance, block, budget)? {
        return Ok(false);
    }
    if role != SemanticEdgeRoleV1::CallReturn {
        return Ok(true);
    }
    budget.charge_work(1)?;
    match instances.call_control(ProductionCallOccurrenceV1 {
        caller: instance,
        block: SemanticBlockIdV1::from_index(block.get()),
    }) {
        Some(ProductionCallControlV1::MayReturn) => Ok(true),
        Some(ProductionCallControlV1::NoNormalReturn) => Ok(false),
        _ => Err(execution_identity_error_v1()),
    }
}

fn execution_identity_edge_active_v1(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    edge: SsaEdgeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let occurrences = instances
        .occurrences(instance)
        .ok_or_else(execution_identity_error_v1)?;
    let edges = occurrences.successors();
    charge_execution_cfg_lookup_v29(edges.len(), budget)?;
    let ordinal = edges
        .binary_search_by_key(&edge, |row| row.id())
        .map_err(|_| execution_identity_error_v1())?;
    execution_identity_successor_active_v1(
        instances,
        instance,
        edge.source(),
        edges[ordinal].edge().role(),
        budget,
    )
}

impl<'a, 'source> ExecutionIdentitySourceIndexV1<'a, 'source> {
    fn new(
        instances: &'a ExecutionInstancesV29<'source>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let mut result = Self {
            instances,
            rows: Vec::new(),
            values: BTreeMap::new(),
            retained: BTreeMap::new(),
            headers: BTreeMap::new(),
            tails: BTreeMap::new(),
            events: emission_vec_v1(instances.instances().len(), budget)?,
            incoming: BTreeMap::new(),
            calls: BTreeMap::new(),
            channels: 0,
        };
        for ordinal in 0..instances.instances().len() {
            budget.charge_work(3)?;
            let instance = instances
                .id_at(ordinal)
                .ok_or_else(execution_identity_error_v1)?;
            budget.charge_work(1)?;
            if !instances
                .instance_reachable(instance)
                .ok_or_else(execution_identity_error_v1)?
            {
                result.events.push(Vec::new());
                continue;
            }
            let row = instances
                .instance(instance)
                .ok_or_else(execution_identity_error_v1)?;
            let occurrences = instances
                .occurrences(instance)
                .ok_or_else(execution_identity_error_v1)?;
            for (index, call) in instances
                .calls(instance)
                .ok_or_else(execution_identity_error_v1)?
                .iter()
                .enumerate()
            {
                budget.charge_work(2)?;
                let occurrence = call.occurrence();
                if occurrence.caller != instance {
                    return Err(execution_identity_error_v1());
                }
                if !execution_identity_block_active_v1(
                    instances,
                    instance,
                    SsaBlockIdV1::new(occurrence.block.index()),
                    budget,
                )? {
                    continue;
                }
                reserve_execution_cfg_map_entry_v29::<(usize, u32), usize>(
                    result.calls.len(),
                    budget,
                )?;
                if result
                    .calls
                    .insert((instance.index(), occurrence.block.index()), index)
                    .is_some()
                {
                    return Err(execution_identity_error_v1());
                }
            }
            let mut events = emission_vec_v1(occurrences.events().len(), budget)?;
            for (index, event) in occurrences.events().iter().enumerate() {
                budget.charge_work(3)?;
                events.push(UnitLocalSourceIndexV1 {
                    key: unit_local_source_key_v1(
                        event.site(),
                        event.operand(),
                        Some(event.role()),
                    ),
                    index,
                });
                let block = match event.site() {
                    ExecutionSiteV29::Statement { block, .. }
                    | ExecutionSiteV29::Terminator { block } => block,
                };
                if !event.is_reachable()
                    || !execution_identity_block_active_v1(instances, instance, block, budget)?
                {
                    continue;
                }
                if let Some(resolved) = event.resolved()
                    && occurrences
                        .terminal_failure_start(block)
                        .is_none_or(|start| (event.ordinal() as usize) < start)
                {
                    budget.charge_work(3)?;
                    let (variable, value) = match resolved {
                        SsaResolvedEventV1::Use { variable, value }
                        | SsaResolvedEventV1::Define { variable, value } => (variable, Some(value)),
                        SsaResolvedEventV1::Kill { variable, .. } => (variable, None),
                    };
                    if !event.is_promoted() || variable != event.event().variable() {
                        return Err(execution_identity_error_v1());
                    }
                    let key = (instance.index(), block.get(), variable.get());
                    charge_execution_cfg_lookup_v29(result.tails.len(), budget)?;
                    if !result.tails.contains_key(&key) {
                        reserve_execution_cfg_map_entry_v29::<(usize, u32, u32), Option<SsaValueV1>>(
                            result.tails.len(),
                            budget,
                        )?;
                    }
                    result.tails.insert(key, value);
                }
                if let Some(SsaResolvedEventV1::Define { variable, value }) = event.resolved() {
                    if !event.is_promoted()
                        || event.role() != ExecutionEventV29::DestinationDefine
                        || variable != event.event().variable()
                    {
                        return Err(execution_identity_error_v1());
                    }
                    result.push(
                        instance,
                        Some(value),
                        SemanticLocalIdV1::from_index(variable.get()),
                        ExecutionIdentitySourceKindV1::Event(index),
                        budget,
                    )?;
                }
            }
            unit_local_source_sort_v1(&mut events, budget)?;
            result.events.push(events);
            for (index, edge) in occurrences.successors().iter().enumerate() {
                budget.charge_work(2)?;
                if !row.ssa().plan().is_reachable(edge.id().source())
                    || !execution_identity_successor_active_v1(
                        instances,
                        instance,
                        edge.id().source(),
                        edge.edge().role(),
                        budget,
                    )?
                {
                    continue;
                }
                let key = (instance.index(), edge.edge().target().index());
                charge_execution_cfg_lookup_v29(result.incoming.len(), budget)?;
                if !result.incoming.contains_key(&key) {
                    reserve_execution_cfg_map_entry_v29::<(usize, u32), Vec<usize>>(
                        result.incoming.len(),
                        budget,
                    )?;
                    result.incoming.insert(key, Vec::new());
                }
                emission_push_v1(
                    result
                        .incoming
                        .get_mut(&key)
                        .ok_or_else(execution_identity_error_v1)?,
                    index,
                    budget,
                )?;
            }
            for (index, entry) in occurrences.entry_definitions().iter().enumerate() {
                budget.charge_work(2)?;
                if let Some(value) = entry.value() {
                    result.push(
                        instance,
                        Some(value),
                        SemanticLocalIdV1::from_index(entry.variable().get()),
                        ExecutionIdentitySourceKindV1::Entry(index),
                        budget,
                    )?;
                }
            }
            for (index, edge) in occurrences.edge_definitions().iter().enumerate() {
                budget.charge_work(2)?;
                if edge.is_reachable()
                    && execution_identity_edge_active_v1(instances, instance, edge.edge(), budget)?
                    && let Some(value) = edge.value()
                {
                    if !edge.is_promoted() {
                        return Err(execution_identity_error_v1());
                    }
                    result.push(
                        instance,
                        Some(value),
                        SemanticLocalIdV1::from_index(edge.variable().get()),
                        ExecutionIdentitySourceKindV1::Edge(index),
                        budget,
                    )?;
                }
            }
            for &block in row.ssa().plan().reverse_postorder() {
                if !execution_identity_block_active_v1(instances, instance, block, budget)? {
                    continue;
                }
                let phis = row
                    .ssa()
                    .plan()
                    .transport_variables(block)
                    .ok_or_else(execution_identity_error_v1)?;
                for &variable in row
                    .ssa()
                    .plan()
                    .live_in(block)
                    .ok_or_else(execution_identity_error_v1)?
                {
                    budget.charge_work(argument_sum_v1(&[phis.len(), 1])?)?;
                    let is_phi = phis.contains(&variable);
                    let index = result.rows.len();
                    result.push(
                        instance,
                        is_phi.then_some(SsaValueV1::BlockArgument { block, variable }),
                        SemanticLocalIdV1::from_index(variable.get()),
                        if is_phi {
                            ExecutionIdentitySourceKindV1::Phi(block)
                        } else {
                            ExecutionIdentitySourceKindV1::Header(block)
                        },
                        budget,
                    )?;
                    if result.rows.len() != index {
                        reserve_execution_cfg_map_entry_v29::<(usize, u32, u32), usize>(
                            result.headers.len(),
                            budget,
                        )?;
                        if result
                            .headers
                            .insert((instance.index(), block.get(), variable.get()), index)
                            .is_some()
                        {
                            return Err(execution_identity_error_v1());
                        }
                    }
                }
            }
        }
        Ok(result)
    }

    fn push(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        value: Option<SsaValueV1>,
        local: SemanticLocalIdV1,
        kind: ExecutionIdentitySourceKindV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let ty = self
            .instances
            .instance(instance)
            .and_then(|instance| instance.declaration().locals().get(local.index() as usize))
            .ok_or_else(execution_identity_error_v1)?
            .ty();
        let count = execution_identity_channels_v1(
            self.instances.owner().source_semantic().types(),
            ty,
            budget,
        )?;
        if count == 0 {
            return Ok(());
        }
        let next = argument_sum_v1(&[self.channels, count])?;
        if let Some(value) = value {
            reserve_execution_cfg_map_entry_v29::<(usize, SsaValueV1), usize>(
                self.values.len(),
                budget,
            )?;
            if self
                .values
                .insert((instance.index(), value), self.rows.len())
                .is_some()
            {
                return Err(execution_identity_error_v1());
            }
        }
        emission_push_v1(
            &mut self.rows,
            ExecutionIdentitySourceRowV1 {
                instance,
                value,
                local,
                selection: ExecutionIdentitySelectionV1 {
                    ty,
                    first: self.channels,
                    count,
                },
                kind,
            },
            budget,
        )?;
        self.channels = next;
        Ok(())
    }

    fn value(
        &self,
        instance: ProductionCallInstanceIdV1,
        value: SsaValueV1,
        local: SemanticLocalIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<ExecutionIdentitySelectionV1, ProductionSemanticKirErrorV1> {
        charge_execution_cfg_lookup_v29(self.values.len(), budget)?;
        let index = self
            .values
            .get(&(instance.index(), value))
            .ok_or_else(execution_identity_error_v1)?;
        let row = self
            .rows
            .get(*index)
            .ok_or_else(execution_identity_error_v1)?;
        if row.instance != instance || row.value != Some(value) || row.local != local {
            return Err(execution_identity_error_v1());
        }
        Ok(row.selection)
    }

    fn header(
        &self,
        instance: ProductionCallInstanceIdV1,
        block: SsaBlockIdV1,
        local: SemanticLocalIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<ExecutionIdentitySelectionV1, ProductionSemanticKirErrorV1> {
        charge_execution_cfg_lookup_v29(self.headers.len(), budget)?;
        let index = self
            .headers
            .get(&(instance.index(), block.get(), local.index()))
            .ok_or_else(execution_identity_error_v1)?;
        let row = self
            .rows
            .get(*index)
            .ok_or_else(execution_identity_error_v1)?;
        if row.instance != instance
            || row.local != local
            || !matches!(row.kind,
            ExecutionIdentitySourceKindV1::Header(original) | ExecutionIdentitySourceKindV1::Phi(original)
            if original == block)
        {
            return Err(execution_identity_error_v1());
        }
        Ok(row.selection)
    }

    fn edge_input(
        &self,
        instance: ProductionCallInstanceIdV1,
        edge: fe2o3_mir_model::SsaEdgeIdV1,
        local: SemanticLocalIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<ExecutionIdentitySelectionV1, ProductionSemanticKirErrorV1> {
        if !execution_identity_edge_active_v1(self.instances, instance, edge, budget)? {
            return Err(execution_identity_error_v1());
        }
        let row = self
            .instances
            .instance(instance)
            .ok_or_else(execution_identity_error_v1)?;
        let definitions = row
            .ssa()
            .plan()
            .edge_definitions(edge)
            .ok_or_else(execution_identity_error_v1)?;
        budget.charge_work(definitions.len())?;
        if let Some(definition) = definitions
            .iter()
            .find(|definition| definition.variable().get() == local.index())
        {
            return self.value(instance, definition.value(), local, budget);
        }
        charge_execution_cfg_lookup_v29(self.tails.len(), budget)?;
        match self
            .tails
            .get(&(instance.index(), edge.source().get(), local.index()))
        {
            Some(Some(value)) => self.value(instance, *value, local, budget),
            // A real common-tail kill is not a pass-through. Failure-only
            // events were excluded using the original captured boundary.
            Some(None) => Err(execution_identity_error_v1()),
            None => self.header(instance, edge.source(), local, budget),
        }
    }

    fn source_use(
        &self,
        instance: ProductionCallInstanceIdV1,
        site: ExecutionSiteV29,
        operand: ExecutionOperandV29,
        local: SemanticLocalIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<ExecutionIdentitySelectionV1, ProductionSemanticKirErrorV1> {
        let block = match site {
            ExecutionSiteV29::Statement { block, .. } | ExecutionSiteV29::Terminator { block } => {
                block
            }
        };
        if !execution_identity_block_active_v1(self.instances, instance, block, budget)? {
            return Err(execution_identity_error_v1());
        }
        let index = self
            .events
            .get(instance.index())
            .ok_or_else(execution_identity_error_v1)?;
        let key = unit_local_source_key_v1(site, operand, Some(ExecutionEventV29::BaseUse));
        let (mut left, mut right) = (0, index.len());
        while left < right {
            budget.charge_work(8)?;
            let middle = left + (right - left) / 2;
            match index[middle].key.cmp(&key) {
                std::cmp::Ordering::Less => left = middle + 1,
                std::cmp::Ordering::Greater => right = middle,
                std::cmp::Ordering::Equal => {
                    let occurrences = self
                        .instances
                        .occurrences(instance)
                        .ok_or_else(execution_identity_error_v1)?;
                    let event = occurrences
                        .events()
                        .get(index[middle].index)
                        .ok_or_else(execution_identity_error_v1)?;
                    if !event.is_promoted() && event.resolved().is_none() {
                        return self.retained_use(instance, index[middle].index, local, budget);
                    }
                    let Some(SsaResolvedEventV1::Use { variable, value }) = event.resolved() else {
                        return Err(execution_identity_error_v1());
                    };
                    if !event.is_promoted()
                        || !event.is_reachable()
                        || variable.get() != local.index()
                        || event.event().variable() != variable
                        || event.site() != site
                        || event.operand() != operand
                        || event.role() != ExecutionEventV29::BaseUse
                    {
                        return Err(execution_identity_error_v1());
                    }
                    return self.value(instance, value, local, budget);
                }
            }
        }
        Err(execution_identity_error_v1())
    }

    fn place(
        &self,
        instance: ProductionCallInstanceIdV1,
        site: ExecutionSiteV29,
        operand: ExecutionOperandV29,
        place: &SemanticPlaceV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<ExecutionIdentitySelectionV1, ProductionSemanticKirErrorV1> {
        let selection = self.source_use(instance, site, operand, place.local(), budget)?;
        let selection = execution_identity_project_v1(
            self.instances.owner().source_semantic().types(),
            selection,
            place.projections(),
            budget,
        )?;
        if selection.ty != place.ty() {
            return Err(execution_identity_error_v1());
        }
        Ok(selection)
    }

    fn operand(
        &self,
        instance: ProductionCallInstanceIdV1,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        operand: &SemanticOperandV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<ExecutionIdentitySelectionV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                self.place(instance, site, role, place, budget)
            }
            SemanticOperandV1::Constant(_) => Err(execution_identity_error_v1()),
        }
    }
}
