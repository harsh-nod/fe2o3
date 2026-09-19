use fe2o3_mir_model::SsaEdgeIdV1;

#[cfg(test)]
struct ExecutionTestObservationV29 {
    locals: Vec<Option<SemanticValueBindingV1>>,
    bindings: BTreeMap<SsaValueV1, SemanticValueBindingV1>,
}

struct ExecutionCfgEdgeV29 {
    id: SsaEdgeIdV1,
    target: usize,
    claimed: bool,
}

struct ExecutionCfgEntryV29 {
    local: u32,
    ty: SemanticTypeIdV1,
    phi: bool,
    value: Option<SsaValueV1>,
    leaves: std::ops::Range<usize>,
}

// Fixed-capacity projections of this instance's captured source plan. They
// contain nominal holder state only; executable scalar flow stays in KIR.
struct ExecutionCfgV29<'a> {
    types: &'a [SemanticTypeDeclV1],
    has_nominal: bool,
    nominal_locals: Vec<usize>,
    edges: Vec<ExecutionCfgEdgeV29>,
    incoming: Vec<usize>,
    arrived: Vec<usize>,
    ranges: Vec<std::ops::Range<usize>>,
    entries: Vec<ExecutionCfgEntryV29>,
    leaves: Vec<Option<ExecutionCfgLeafV29>>,
}

impl<'a> ExecutionCfgV29<'a> {
    fn new(
        types: &'a [SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        ssa: &ProductionSemanticSsaFunctionPlanV1,
        occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let blocks = function.blocks().len();
        let mut nominal_locals = emission_vec_v1(function.locals().len(), budget)?;
        for local in function.locals() {
            nominal_locals.push(execution_cfg_nominal_count_v29(types, local.ty(), budget)?);
        }
        budget.charge_work(nominal_locals.len())?;
        let has_nominal = nominal_locals.iter().any(|count| *count != 0);
        let mut edges = emission_vec_v1(occurrences.successors().len(), budget)?;
        let mut incoming = emission_vec_v1(blocks, budget)?;
        let mut arrived = emission_vec_v1(blocks, budget)?;
        budget.charge_work(argument_product_v1(blocks, 2)?)?;
        incoming.resize(blocks, 0usize);
        arrived.resize(blocks, 0usize);
        for edge in occurrences.successors() {
            budget.charge_work(2)?;
            if !ssa.plan().is_reachable(edge.id().source()) {
                continue;
            }
            let target = edge.edge().target().index() as usize;
            if target >= blocks
                || edges
                    .last()
                    .is_some_and(|previous: &ExecutionCfgEdgeV29| previous.id >= edge.id())
            {
                return Err(execution_cfg_error_v29());
            }
            incoming[target] = argument_sum_v1(&[incoming[target], 1])?;
            edges.push(ExecutionCfgEdgeV29 {
                id: edge.id(),
                target,
                claimed: false,
            });
        }
        let mut entry_count = 0;
        let mut leaf_count = 0;
        for block in ssa.plan().reverse_postorder() {
            if block.get() == function.entry().index() {
                continue;
            }
            for variable in ssa
                .plan()
                .live_in(*block)
                .ok_or_else(execution_cfg_error_v29)?
            {
                budget.charge_work(1)?;
                let count = nominal_locals[variable.get() as usize];
                if count != 0 {
                    entry_count = argument_sum_v1(&[entry_count, 1])?;
                    leaf_count = argument_sum_v1(&[leaf_count, count])?;
                }
            }
        }
        let mut entries = emission_vec_v1(entry_count, budget)?;
        let mut leaves = emission_vec_v1(leaf_count, budget)?;
        let mut ranges = emission_vec_v1(blocks, budget)?;
        budget.charge_work(leaf_count)?;
        leaves.resize_with(leaf_count, || None);
        let mut next_leaf = 0;
        for block in 0..blocks {
            budget.charge_work(1)?;
            let first = entries.len();
            let id = SsaBlockIdV1::new(block as u32);
            if block != function.entry().index() as usize && ssa.plan().is_reachable(id) {
                let phis = ssa
                    .plan()
                    .transport_variables(id)
                    .ok_or_else(execution_cfg_error_v29)?;
                for variable in ssa.plan().live_in(id).ok_or_else(execution_cfg_error_v29)? {
                    budget.charge_work(argument_sum_v1(&[phis.len(), 1])?)?;
                    let local = variable.get() as usize;
                    let count = nominal_locals[local];
                    if count == 0 {
                        continue;
                    }
                    let end = argument_sum_v1(&[next_leaf, count])?;
                    entries.push(ExecutionCfgEntryV29 {
                        local: variable.get(),
                        ty: function.locals()[local].ty(),
                        phi: phis.contains(variable),
                        value: None,
                        leaves: next_leaf..end,
                    });
                    next_leaf = end;
                }
            }
            ranges.push(first..entries.len());
        }
        if entries.len() != entry_count || next_leaf != leaf_count {
            return Err(execution_cfg_error_v29());
        }
        Ok(Self {
            types,
            has_nominal,
            nominal_locals,
            edges,
            incoming,
            arrived,
            ranges,
            entries,
            leaves,
        })
    }

    fn edge_index(
        &self,
        id: SsaEdgeIdV1,
        target: SemanticBlockIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(self.edges.len().checked_ilog2().unwrap_or(0) as usize + 2)?;
        let index = self
            .edges
            .binary_search_by_key(&id, |edge| edge.id)
            .map_err(|_| execution_cfg_error_v29())?;
        let edge = &self.edges[index];
        if edge.target != target.index() as usize || edge.claimed {
            return Err(execution_cfg_error_v29());
        }
        Ok(index)
    }

    fn enter(
        &self,
        block: SemanticBlockIdV1,
        current: &mut [Option<SsaValueV1>],
        seen: &[bool],
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let block = block.index() as usize;
        let range = self.ranges.get(block).ok_or_else(execution_cfg_error_v29)?;
        budget.charge_work(argument_sum_v1(&[range.len(), 1])?)?;
        if !range.is_empty()
            && (self.incoming[block] == 0 || self.arrived[block] != self.incoming[block])
        {
            return Err(execution_cfg_error_v29());
        }
        for entry in &self.entries[range.clone()] {
            let value = entry.value.ok_or_else(execution_cfg_error_v29)?;
            let local = entry.local as usize;
            if seen[local] && current[local] != Some(value) {
                return Err(execution_cfg_error_v29());
            }
            current[local] = Some(value);
        }
        Ok(())
    }
}

impl ExecutionAvailabilityV29<'_> {
    fn check_cfg_edge_plan(
        &self,
        block: SemanticBlockIdV1,
        ordinal: u32,
        target: SemanticBlockIdV1,
        arguments: &[SsaArgumentV1],
        definitions: &[SsaArgumentV1],
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_ledger(budget)?;
        let id = SsaEdgeIdV1::new(SsaBlockIdV1::new(block.index()), ordinal);
        self.events.complete(budget)?;
        if self.block != Some(id.source()) {
            return Err(execution_cfg_error_v29());
        }
        self.cfg.edge_index(id, target, budget)?;
        budget.charge_work(argument_sum_v1(&[arguments.len(), definitions.len(), 2])?)?;
        if self.ssa.plan().edge_arguments(id) != Some(arguments)
            || self.ssa.plan().edge_definitions(id) != Some(definitions)
        {
            return Err(execution_cfg_error_v29());
        }
        Ok(())
    }

    fn enter_cfg(
        &mut self,
        block: SemanticBlockIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_ledger(budget)?;
        if self.cfg.has_nominal
            && block == self.function.entry()
            && self.cfg.incoming[block.index() as usize] != 0
        {
            return Err(execution_cfg_error_v29());
        }
        self.cfg.enter(block, &mut self.current, &self.seen, budget)
    }

    fn transport_edge(
        &mut self,
        block: SemanticBlockIdV1,
        ordinal: u32,
        target: SemanticBlockIdV1,
        locals: &[Option<SemanticValueBindingV1>],
        archive: &BTreeMap<SsaValueV1, SemanticValueBindingV1>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_ledger(budget)?;
        self.events.complete(budget)?;
        if self.block != Some(SsaBlockIdV1::new(block.index())) {
            return Err(execution_cfg_error_v29());
        }
        let id = SsaEdgeIdV1::new(SsaBlockIdV1::new(block.index()), ordinal);
        let index = self.cfg.edge_index(id, target, budget)?;
        if self.cfg.has_nominal && self.visited[target.index() as usize] {
            return Err(execution_cfg_error_v29());
        }
        let definitions = self
            .ssa
            .plan()
            .edge_definitions(id)
            .ok_or_else(execution_cfg_error_v29)?;
        let arguments = self
            .ssa
            .plan()
            .edge_arguments(id)
            .ok_or_else(execution_cfg_error_v29)?;
        let range = self.cfg.ranges[target.index() as usize].clone();
        for entry in &mut self.cfg.entries[range] {
            budget.charge_work(argument_sum_v1(&[definitions.len(), arguments.len(), 3])?)?;
            let edge_definition = definitions
                .iter()
                .find(|definition| definition.variable().get() == entry.local);
            let value = edge_definition
                .map(|definition| definition.value())
                .or(self.current[entry.local as usize])
                .ok_or_else(execution_cfg_error_v29)?;
            let argument = arguments
                .iter()
                .find(|argument| argument.variable().get() == entry.local);
            if entry.phi != argument.is_some()
                || argument.is_some_and(|argument| argument.value() != value)
            {
                return Err(execution_cfg_error_v29());
            }
            // This is the borrowed owner's exact edge definition, not a
            // source-block-wide definition shared with another successor.
            let held = locals
                .get(entry.local as usize)
                .and_then(Option::as_ref)
                .ok_or_else(execution_cfg_error_v29)?;
            charge_execution_cfg_lookup_v29(archive.len(), budget)?;
            let original = archive.get(&value).ok_or_else(execution_cfg_error_v29)?;
            let mut leaves = self.cfg.leaves[entry.leaves.clone()].iter_mut();
            merge_execution_cfg_binding_v29(
                self.cfg.types,
                entry.ty,
                held,
                original,
                &mut leaves,
                &mut 0,
                budget,
            )?;
            if leaves.next().is_some() {
                return Err(execution_cfg_error_v29());
            }
            let incoming = if entry.phi {
                SsaValueV1::BlockArgument {
                    block: SsaBlockIdV1::new(target.index()),
                    variable: fe2o3_mir_model::SsaVariableIdV1::new(entry.local),
                }
            } else {
                value
            };
            if entry.value.is_some_and(|previous| previous != incoming) {
                return Err(execution_cfg_error_v29());
            }
            entry.value = Some(incoming);
        }
        self.cfg.arrived[target.index() as usize] =
            argument_sum_v1(&[self.cfg.arrived[target.index() as usize], 1])?;
        self.cfg.edges[index].claimed = true;
        Ok(())
    }
}

fn with_execution_cfg_values_v29<R>(
    binding: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
    consume: impl FnOnce(
        &[ValueDef],
        &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    let floor = budget.storage();
    let mut values = Vec::new();
    let built = catch_unwind(AssertUnwindSafe(|| {
        execution_cfg_values_v29(binding, &mut values, &mut 0, budget)
    }));
    let storage = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let result = match built {
        Ok(Ok(())) => catch_unwind(AssertUnwindSafe(|| consume(&values, budget))),
        Ok(Err(error)) => Ok(Err(error)),
        Err(payload) => Err(payload),
    };
    drop(values);
    let release = budget.release_storage(storage);
    match result {
        Ok(result) => {
            release?;
            result
        }
        Err(payload) => {
            let _ = release;
            resume_unwind(payload)
        }
    }
}

impl SemanticFunctionLoweringV1<'_> {
    fn execution_cfg_local_v29(&self, local: usize) -> bool {
        self.execution
            .as_ref()
            .is_some_and(|cursor| cursor.cfg.nominal_locals[local] != 0)
    }

    fn restore_execution_cfg_v29(
        &mut self,
        block: SemanticBlockIdV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.execution.is_none() {
            return Ok(());
        }
        self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_ref()
                .ok_or_else(execution_cfg_error_v29)?;
            cursor.check_ledger(budget)?;
            for entry in &cursor.cfg.entries[cursor.cfg.ranges[block.index() as usize].clone()] {
                let source_value = entry.value.ok_or_else(execution_cfg_error_v29)?;
                let rebuild = |values: &[ValueDef], budget: &mut dyn SemanticEmissionBudgetV1| {
                    let mut leaves = cursor.cfg.leaves[entry.leaves.clone()].iter();
                    let mut values = values.iter();
                    let binding = rebuild_execution_cfg_binding_v29(
                        this.types,
                        entry.ty,
                        entry.phi,
                        &mut leaves,
                        &mut values,
                        &mut 0,
                        budget,
                    )?;
                    if leaves.next().is_some() || values.next().is_some() {
                        return Err(execution_cfg_error_v29());
                    }
                    Ok(binding)
                };
                let binding = if entry.phi {
                    charge_execution_cfg_lookup_v29(this.block_parameters.len(), budget)?;
                    charge_execution_cfg_lookup_v29(this.function.locals().len(), budget)?;
                    let values = this
                        .block_parameters
                        .get(&block.index())
                        .and_then(|locals| locals.get(&entry.local))
                        .ok_or_else(execution_cfg_error_v29)?;
                    let archived = rebuild(values, budget)?;
                    reserve_execution_cfg_archive_v29(this.semantic_ssa_bindings.len(), budget)?;
                    insert_semantic_ssa_binding_v1(
                        &mut this.semantic_ssa_bindings,
                        this.semantic_function.index(),
                        Some(block.index()),
                        None,
                        source_value,
                        archived,
                    )?;
                    rebuild(values, budget)?
                } else {
                    charge_execution_cfg_lookup_v29(this.semantic_ssa_bindings.len(), budget)?;
                    let original = this
                        .semantic_ssa_bindings
                        .get(&source_value)
                        .ok_or_else(execution_cfg_error_v29)?;
                    with_execution_cfg_values_v29(original, budget, rebuild)?
                };
                this.locals[entry.local as usize] = Some(binding);
            }
            Ok(())
        })
    }
}

fn charge_execution_cfg_lookup_v29(
    count: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_product_v1(
        count.checked_ilog2().unwrap_or(0) as usize + 2,
        16,
    )?)
}

fn reserve_execution_cfg_archive_v29(
    count: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    reserve_execution_cfg_map_entry_v29::<SsaValueV1, SemanticValueBindingV1>(count, budget)
}

fn reserve_execution_cfg_map_entry_v29<K, V>(
    count: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    charge_execution_cfg_lookup_v29(count, budget)?;
    // Conservative split-path allowance for the pinned toolchain's BTreeMap:
    // at most one new node per level plus a root. Keep it charged for the request.
    let levels = count.checked_ilog2().unwrap_or(0) as usize + 2;
    let node = argument_product_v1(32, std::mem::size_of::<(K, V, usize)>())?;
    budget.reserve_storage(argument_product_v1(levels, node)?)
}

fn clone_execution_cfg_parameters_v29(
    parameters: &BTreeMap<u32, Vec<ValueDef>>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<BTreeMap<u32, Vec<ValueDef>>, ProductionSemanticKirErrorV1> {
    let mut output = BTreeMap::new();
    for (local, values) in parameters {
        let mut components = emission_vec_v1(values.len(), budget)?;
        for value in values {
            components.push(ValueDef::new(
                value.id,
                execution_cfg_clone_type_v29(&value.ty, budget)?,
            ));
        }
        reserve_execution_cfg_map_entry_v29::<u32, Vec<ValueDef>>(output.len(), budget)?;
        output.insert(*local, components);
    }
    Ok(output)
}

fn clone_execution_cfg_arguments_v29(
    arguments: &[SsaArgumentV1],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<SsaArgumentV1>, ProductionSemanticKirErrorV1> {
    let mut output = emission_vec_v1(arguments.len(), budget)?;
    budget.charge_work(argument_product_v1(arguments.len(), 2)?)?;
    output.extend_from_slice(arguments);
    Ok(output)
}
