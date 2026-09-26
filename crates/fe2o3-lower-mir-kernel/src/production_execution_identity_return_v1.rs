// A returned identity is selected from an actual prepared source input. The
// source equations select it; every emitted normal return must still observe it.
struct ExecutionIdentityReturnInputV1 {
    local: SemanticLocalIdV1,
    argument: u32,
    tuple_field: Option<u32>,
    selection: ExecutionIdentitySelectionV1,
}

struct ExecutionIdentityReturnSourceV1 {
    output: ExecutionIdentitySelectionV1,
    leaves: Vec<ExecutionIdentityReturnLeafV1>,
    inputs: Vec<ExecutionIdentityReturnInputV1>,
    exits: Vec<ExecutionIdentityReturnExitV1>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct ExecutionIdentityReturnExitV1 {
    block: SemanticBlockIdV1,
    local: SemanticLocalIdV1,
}

struct ExecutionIdentityReturnLeafV1 {
    ty: SemanticTypeIdV1,
    first: usize,
    borrowed: bool,
}

fn execution_identity_return_leaf_layout_v1(
    types: &[SemanticTypeDeclV1],
    selection: ExecutionIdentitySelectionV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<ExecutionIdentityReturnLeafV1>, ProductionSemanticKirErrorV1> {
    budget.reserve_storage(std::mem::size_of::<Vec<SemanticTypeIdV1>>())?;
    let mut pending = emission_vec_v1(1, budget)?;
    pending.push(selection.ty);
    let mut output = Vec::new();
    let mut first = selection.first;
    let mut nodes = 0;
    while let Some(ty) = pending.pop() {
        execution_cfg_charge_node_v29(&mut nodes, budget)?;
        if let Some(borrowed) = execution_cfg_nominal_kind_v29(types, ty)? {
            emission_push_v1(
                &mut output,
                ExecutionIdentityReturnLeafV1 {
                    ty,
                    first,
                    borrowed,
                },
                budget,
            )?;
            first = argument_sum_v1(&[first, if borrowed { 2 } else { 1 }])?;
        } else if let Some((count, fields, element)) = execution_cfg_fields_v29(types, ty)? {
            for field in (0..count).rev() {
                emission_push_v1(
                    &mut pending,
                    fields.map_or(element, |fields| fields[field]),
                    budget,
                )?;
            }
        }
    }
    if first != argument_sum_v1(&[selection.first, selection.count])? {
        return Err(execution_identity_error_v1());
    }
    let bytes = argument_sum_v1(&[
        std::mem::size_of::<Vec<SemanticTypeIdV1>>(),
        argument_product_v1(pending.capacity(), std::mem::size_of::<SemanticTypeIdV1>())?,
    ])?;
    drop(pending);
    budget.release_storage(bytes)?;
    Ok(output)
}

struct ExecutionIdentityReturnObservationV1 {
    source: SemanticBlockIdV1,
    target: BlockId,
    values: Vec<ValueId>,
    nominal: Vec<Option<ExecutionCfgLeafV29>>,
}

struct ExecutionIdentityReturnWitnessV1 {
    expected: Vec<Option<ExecutionCfgLeafV29>>,
    observations: Vec<ExecutionIdentityReturnSlotV1>,
}

struct ExecutionIdentityReturnSlotV1 {
    exit: ExecutionIdentityReturnExitV1,
    observation: Option<ExecutionIdentityReturnObservationV1>,
}

type PreparedExecutionDefinedCallV1 = (FunctionId, Vec<ValueId>, Vec<Option<ExecutionCfgLeafV29>>);

fn execution_identity_return_present_v1(
    instances: &ExecutionInstancesV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let mut present = false;
    for (ordinal, row) in instances.instances().iter().enumerate().skip(1) {
        budget.charge_work(3)?;
        let id = instances.id_at(ordinal).ok_or_else(execution_identity_error_v1)?;
        if !instances.instance_reachable(id).ok_or_else(execution_identity_error_v1)?
            || !instances.instance_may_return(id).ok_or_else(execution_identity_error_v1)?
        {
            continue;
        }
        present |= execution_identity_channels_v1(
            instances.owner().source_semantic().types(),
            row.declaration().abi().source_output_type(),
            budget,
        )? != 0;
    }
    Ok(present)
}

fn execution_identity_return_sources_v1(
    index: &ExecutionIdentitySourceIndexV1<'_, '_>,
    roots: &mut Vec<usize>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<Option<ExecutionIdentityReturnSourceV1>>, ProductionSemanticKirErrorV1> {
    let instances = index.instances;
    let mut returns: Vec<Option<ExecutionIdentityReturnSourceV1>> =
        emission_vec_v1(instances.instances().len(), budget)?;
    budget.charge_work(instances.instances().len())?;
    returns.resize_with(instances.instances().len(), || None);
    for (ordinal, row) in index.rows.iter().enumerate() {
        budget.charge_work(3)?;
        let ExecutionIdentitySourceKindV1::Edge(definition) = row.kind else {
            continue;
        };
        let occurrences = instances
            .occurrences(row.instance)
            .ok_or_else(execution_identity_error_v1)?;
        let definition = occurrences
            .edge_definitions()
            .get(definition)
            .ok_or_else(execution_identity_error_v1)?;
        charge_execution_cfg_lookup_v29(index.calls.len(), budget)?;
        let call = *index
            .calls
            .get(&(row.instance.index(), definition.edge().source().get()))
            .ok_or_else(execution_identity_error_v1)?;
        let call = instances
            .calls(row.instance)
            .and_then(|calls| calls.get(call))
            .ok_or_else(execution_identity_error_v1)?;
        let Some(child) = call.child() else { continue };
        let function = instances
            .instance(child)
            .ok_or_else(execution_identity_error_v1)?
            .declaration();
        if row.selection.ty != function.abi().source_output_type()
            || definition.value() != row.value
            || definition.variable().get() != row.local.index()
            || returns[child.index()].is_some()
        {
            return Err(execution_identity_error_v1());
        }
        let mut exits = Vec::new();
        for exit in instances
            .exits(child)
            .ok_or_else(execution_identity_error_v1)?
        {
            budget.charge_work(2)?;
            if !instances.block_reachable(child, exit.block).ok_or_else(execution_identity_error_v1)? {
                continue;
            }
            if let production_call_instances_v1::ProductionInstanceExitKindV1::Return { local } =
                exit.kind
            {
                if exit.instance != child {
                    return Err(execution_identity_error_v1());
                }
                emission_push_v1(
                    &mut exits,
                    ExecutionIdentityReturnExitV1 {
                        block: exit.block,
                        local,
                    },
                    budget,
                )?;
            }
        }
        if exits.is_empty() {
            return Err(execution_identity_error_v1());
        }
        // The original exit roster is RPO; this sparse lookup roster is keyed
        // by exact source block without changing original emission order.
        call_splice_sort_work_v1(exits.len(), budget).map_err(|error| match error {
            CallInstanceEmissionErrorV1::Resource(error) => error.into(),
            _ => execution_identity_error_v1(),
        })?;
        exits.sort_unstable_by_key(|exit| exit.block.index());
        budget.charge_work(exits.len())?;
        if exits.windows(2).any(|pair| pair[0].block >= pair[1].block) {
            return Err(execution_identity_error_v1());
        }
        emission_push_v1(roots, ordinal, budget)?;
        returns[child.index()] = Some(ExecutionIdentityReturnSourceV1 {
            output: row.selection,
            leaves: execution_identity_return_leaf_layout_v1(
                instances.owner().source_semantic().types(),
                row.selection,
                budget,
            )?,
            inputs: Vec::new(),
            exits,
        });
    }
    for (ordinal, row) in index.rows.iter().enumerate() {
        budget.charge_work(2)?;
        if !matches!(row.kind, ExecutionIdentitySourceKindV1::Entry(_)) {
            continue;
        }
        let Some(returned) = returns[row.instance.index()].as_mut() else {
            continue;
        };
        let selector = instances
            .parameter_source(row.instance, row.local, budget)
            .map_err(|error| match error {
                production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => {
                    error.into()
                }
                _ => execution_identity_error_v1(),
            })?;
        let incoming = instances
            .incoming(row.instance)
            .ok_or_else(execution_identity_error_v1)?;
        if selector.ty != row.selection.ty
            || incoming
                .source()
                .arguments()
                .get(selector.source_argument as usize)
                .is_none_or(|operand| !std::ptr::eq(operand, selector.operand))
        {
            return Err(execution_identity_error_v1());
        }
        emission_push_v1(
            &mut returned.inputs,
            ExecutionIdentityReturnInputV1 {
                local: row.local,
                argument: selector.source_argument,
                tuple_field: selector.tuple_field,
                selection: row.selection,
            },
            budget,
        )?;
        emission_push_v1(roots, ordinal, budget)?;
    }
    Ok(returns)
}

impl ExecutionIdentityPlanV1<'_, '_> {
    fn prepare_return_transfer(
        &self,
        cursor: &ExecutionAvailabilityV29<'_>,
        child: ProductionCallInstanceIdV1,
        prepared: &PreparedDefinedCallArgumentsV1<'_>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<ExecutionIdentityReturnWitnessV1>, ProductionSemanticKirErrorV1> {
        self.prepare_return_obligation_v1(cursor, child, prepared, None, budget)
    }

    fn prepare_return_obligation_v1(
        &self,
        cursor: &ExecutionAvailabilityV29<'_>,
        child: ProductionCallInstanceIdV1,
        prepared: &PreparedDefinedCallArgumentsV1<'_>,
        deferred_inputs: Option<&mut ExecutionReturnInputsV1>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<ExecutionIdentityReturnWitnessV1>, ProductionSemanticKirErrorV1> {
        self.check_cursor(cursor, budget)?;
        budget.charge_work(12)?;
        let Some(source) = self.returns.get(child.index()).and_then(Option::as_ref) else {
            return Ok(None);
        };
        let incoming = self
            .index
            .instances
            .incoming(child)
            .ok_or_else(execution_identity_error_v1)?;
        let origin = prepared
            .execution
            .as_ref()
            .ok_or_else(execution_identity_error_v1)?;
        if incoming.child() != Some(child)
            || incoming.occurrence().caller != cursor.instance
            || origin.occurrence != incoming.occurrence()
            || origin.source != self.source
            || origin.ledger != self.ledger
            || origin.scope.ledger != self.ledger
            || origin.function != cursor.function_id
            || self
                .index
                .instances
                .instance(child)
                .map(|row| row.function())
                != Some(origin.callee)
            || prepared.source_bindings.len() != incoming.source().arguments().len()
        {
            return Err(execution_identity_error_v1());
        }
        let original = self
            .index
            .instances
            .instance(child)
            .ok_or_else(execution_identity_error_v1)?;
        let mut next = source.output.first;
        if source.output.ty != original.declaration().abi().source_output_type() {
            return Err(execution_identity_error_v1());
        }
        for leaf in &source.leaves {
            budget.charge_work(2)?;
            if leaf.first != next {
                return Err(execution_identity_error_v1());
            }
            next = argument_sum_v1(&[next, if leaf.borrowed { 2 } else { 1 }])?;
        }
        if next != argument_sum_v1(&[source.output.first, source.output.count])? {
            return Err(execution_identity_error_v1());
        }
        let references = cursor.references.ok_or_else(execution_identity_error_v1)?;
        references.check(budget)?;
        if !std::ptr::eq(references.plan, self.references) {
            return Err(execution_identity_error_v1());
        }
        budget.reserve_storage(std::mem::size_of::<Option<ExecutionIdentityReturnWitnessV1>>())?;
        let mut expected: Vec<Option<ExecutionCfgLeafV29>> =
            emission_vec_v1(source.leaves.len(), budget)?;
        budget.charge_work(source.leaves.len())?;
        expected.resize_with(source.leaves.len(), || None);
        let scratch = budget.storage();
        budget.reserve_storage(std::mem::size_of::<
            BTreeMap<(usize, Option<usize>), ExecutionCfgLeafV29>,
        >())?;
        let mut inputs: BTreeMap<(usize, Option<usize>), ExecutionCfgLeafV29> = BTreeMap::new();
        for input in &source.inputs {
            budget.charge_work(4)?;
            let binding = prepared
                .source_bindings
                .get(input.argument as usize)
                .ok_or_else(execution_identity_error_v1)?;
            let binding = match (binding, input.tuple_field) {
                (SemanticValueBindingV1::Aggregate(fields), Some(field)) => fields
                    .get(field as usize)
                    .ok_or_else(execution_identity_error_v1)?,
                (_, None) => binding,
                _ => return Err(execution_identity_error_v1()),
            };
            let node =
                source_reference_entry_node_v29(references.plan, child, input.local, None, budget)?
                    .ok_or_else(execution_identity_error_v1)?;
            if references.plan.nodes[node].ty != input.selection.ty {
                return Err(execution_identity_error_v1());
            }
            let (leaves, values) =
                source_reference_call_shape_v29(references, node, binding, budget)?;
            let mut component = input.selection.first;
            for leaf in &leaves {
                let class = self.class(component, budget)?;
                if !matches!(
                    leaf,
                    Some(ExecutionCfgLeafV29::Owned(_) | ExecutionCfgLeafV29::Borrow(_))
                ) {
                    return Err(execution_identity_error_v1());
                }
                let leaf = leaf.as_ref().ok_or_else(execution_identity_error_v1)?;
                let referent = if matches!(leaf, ExecutionCfgLeafV29::Borrow(_)) {
                    Some(self.class(argument_sum_v1(&[component, 1])?, budget)?)
                } else {
                    None
                };
                component = argument_sum_v1(&[component, if referent.is_some() { 2 } else { 1 }])?;
                let key = (class, referent);
                charge_execution_cfg_lookup_v29(inputs.len(), budget)?;
                if let Some(previous) = inputs.get(&key) {
                    budget.charge_work(1)?;
                    if previous != leaf {
                        return Err(execution_identity_error_v1());
                    }
                } else {
                    reserve_execution_cfg_map_entry_v29::<
                        (usize, Option<usize>),
                        ExecutionCfgLeafV29,
                    >(inputs.len(), budget)?;
                    inputs.insert(key, leaf.clone());
                }
            }
            if component != argument_sum_v1(&[input.selection.first, input.selection.count])? {
                return Err(execution_identity_error_v1());
            }
            drop((leaves, values));
        }
        for (layout, slot) in source.leaves.iter().zip(&mut expected) {
            let class = self.class(layout.first, budget)?;
            let referent = if layout.borrowed {
                Some(self.class(argument_sum_v1(&[layout.first, 1])?, budget)?)
            } else {
                None
            };
            charge_execution_cfg_lookup_v29(inputs.len(), budget)?;
            let Some(leaf) = inputs.get(&(class, referent)) else {
                if deferred_inputs.is_some() {
                    // This slot is an unresolved obligation, not a binding. The
                    // pending-return owner cannot resume its caller until the
                    // actual child archive/producer/return census completes it.
                    continue;
                }
                return Err(execution_identity_error_v1());
            };
            match leaf {
                ExecutionCfgLeafV29::Owned(binding) if !layout.borrowed => {
                    binding.check_type(cursor.cfg.types, layout.ty)
                }
                ExecutionCfgLeafV29::Borrow(binding) if layout.borrowed => {
                    binding.check_type(cursor.cfg.types, layout.ty)
                }
                _ => return Err(execution_identity_error_v1()),
            }
            .map_err(|_| execution_identity_error_v1())?;
            *slot = Some(leaf.clone());
        }
        let captured = match deferred_inputs {
            Some(retained) => retained.capture(&inputs, budget)?,
            None => 0,
        };
        drop(inputs);
        // These bounded, callback-free readers cannot grow the source arena.
        // Keep every transient input walk paid until the class map is dropped.
        budget.release_storage(
            budget
                .storage()
                .checked_sub(scratch)
                .and_then(|bytes| bytes.checked_sub(captured))
                .ok_or(ArgumentResourceV1::Accounting)?,
        )?;
        let mut observations = emission_vec_v1(source.exits.len(), budget)?;
        budget.charge_work(source.exits.len())?;
        observations.extend(source.exits.iter().copied().map(|exit| {
            ExecutionIdentityReturnSlotV1 {
                exit,
                observation: None,
            }
        }));
        Ok(Some(ExecutionIdentityReturnWitnessV1 {
            expected,
            observations,
        }))
    }
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn consume_execution_return_source_v1(
        &mut self,
        block: SemanticBlockIdV1,
        local: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !self.execution_cfg_local_v29(local) {
            return Ok(());
        }
        self.with_emission_budget_v1(|this, budget| {
            budget.charge_work(6)?;
            if !matches!(
                this.function
                    .blocks()
                    .get(block.index() as usize)
                    .map(|row| row.terminator().kind()),
                Some(SemanticTerminatorKindV1::Return)
            ) || this.function.locals().get(local).map(|row| row.role())
                != Some(SemanticLocalRoleV1::Return)
            {
                return Err(execution_identity_error_v1());
            }
            budget.reserve_storage(std::mem::size_of::<SemanticPlaceV1>())?;
            let place = SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(
                    u32::try_from(local).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                ),
                Vec::new(),
                this.function.locals()[local].ty(),
            )
            .map_err(|_| execution_identity_error_v1())?;
            let cursor = this
                .execution
                .as_mut()
                .ok_or_else(execution_identity_error_v1)?;
            let value = cursor.use_place(
                execution_site_v29(block, None),
                ExecutionOperandV29::ReturnValue,
                &place,
                false,
                budget,
            )?;
            check_execution_archive_v29(
                &this.locals,
                &this.semantic_ssa_bindings,
                &place,
                value,
                budget,
            )?;
            drop(place);
            budget.release_storage(std::mem::size_of::<SemanticPlaceV1>())?;
            Ok(())
        })
    }

    fn observe_execution_return_v1(
        &mut self,
        block: SemanticBlockIdV1,
        values: &[ValueId],
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let Some(consumer) = self.execution_calls.take() else {
            return Ok(());
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            consumer.observe_return(self, block, values)
        }));
        self.execution_calls = Some(consumer);
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

impl ExecutionIdentityReturnWitnessV1 {
    fn observe(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_, '_>,
        block: SemanticBlockIdV1,
        values: &[ValueId],
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.observe_pending_v1(lowering, block, values, false)
    }

    fn observe_pending_v1(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_, '_>,
        block: SemanticBlockIdV1,
        values: &[ValueId],
        pending_completion: bool,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let target = lowering.kernel_block_id_v1(block)?;
        lowering.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_ref()
                .ok_or_else(execution_identity_error_v1)?;
            let (plan, instance) = cursor.identities.ok_or_else(execution_identity_error_v1)?;
            plan.check_cursor(cursor, budget)?;
            let source = plan
                .returns
                .get(instance.index())
                .and_then(Option::as_ref)
                .ok_or_else(execution_identity_error_v1)?;
            budget.charge_work(6)?;
            charge_execution_cfg_lookup_v29(source.exits.len(), budget)?;
            let slot = source
                .exits
                .binary_search_by_key(&block.index(), |exit| exit.block.index())
                .map_err(|_| execution_identity_error_v1())?;
            let exit = source.exits[slot];
            let observed = self
                .observations
                .get(slot)
                .ok_or_else(execution_identity_error_v1)?;
            if observed.exit != exit
                || observed.observation.is_some()
                || self.observations.len() != source.exits.len()
                || cursor.block != Some(SsaBlockIdV1::new(block.index()))
            {
                return Err(execution_identity_error_v1());
            }
            let local = exit.local.index() as usize;
            if this.function.locals().get(local).map(|row| row.role())
                != Some(SemanticLocalRoleV1::Return)
            {
                return Err(execution_identity_error_v1());
            }
            // This read reauthenticates an already claimed source use. Ordinary
            // find_event deliberately refuses claimed events and stays unchanged.
            let key = unit_local_source_key_v1(
                execution_site_v29(block, None),
                ExecutionOperandV29::ReturnValue,
                Some(ExecutionEventV29::BaseUse),
            );
            charge_execution_cfg_lookup_v29(cursor.index.len(), budget)?;
            let indexed = cursor
                .index
                .binary_search_by_key(&key, |row| row.key)
                .map_err(|_| execution_identity_error_v1())?;
            let ordinal = cursor.index[indexed].index;
            let event = cursor
                .occurrences
                .events()
                .get(ordinal)
                .ok_or_else(execution_identity_error_v1)?;
            let Some(SsaResolvedEventV1::Use { variable, value }) = event.resolved() else {
                return Err(execution_identity_error_v1());
            };
            if event.site() != execution_site_v29(block, None)
                || event.operand() != ExecutionOperandV29::ReturnValue
                || event.role() != ExecutionEventV29::BaseUse
                || !event.is_promoted()
                || !event.is_reachable()
                || variable.get() as usize != local
                || cursor.claimed.get(ordinal) != Some(&true)
                || cursor.current.get(local) != Some(&Some(value))
            {
                return Err(execution_identity_error_v1());
            }
            let binding = this
                .locals
                .get(local)
                .and_then(Option::as_ref)
                .ok_or_else(execution_identity_error_v1)?;
            budget.reserve_storage(std::mem::size_of::<SemanticPlaceV1>())?;
            let place = SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(variable.get()),
                Vec::new(),
                this.function.locals()[local].ty(),
            )
            .map_err(|_| execution_identity_error_v1())?;
            check_execution_archive_v29(
                &this.locals,
                &this.semantic_ssa_bindings,
                &place,
                value,
                budget,
            )?;
            drop(place);
            budget.release_storage(std::mem::size_of::<SemanticPlaceV1>())?;
            let references = cursor.references.ok_or_else(execution_identity_error_v1)?;
            let node = references
                .plan
                .returns
                .get(instance.index())
                .copied()
                .flatten()
                .ok_or_else(execution_identity_error_v1)?;
            let scratch = budget.storage();
            let (leaves, physical) =
                source_reference_call_shape_v29(references, node, binding, budget)?;
            budget.charge_work(argument_sum_v1(&[leaves.len(), self.expected.len()])?)?;
            if pending_completion {
                if leaves.len() != self.expected.len() {
                    return Err(execution_identity_error_v1());
                }
                for (observed, expected) in leaves.iter().zip(&self.expected) {
                    if !matches!(
                        observed,
                        Some(ExecutionCfgLeafV29::Owned(_) | ExecutionCfgLeafV29::Borrow(_))
                    ) || expected
                        .as_ref()
                        .is_some_and(|expected| observed.as_ref() != Some(expected))
                    {
                        return Err(execution_identity_error_v1());
                    }
                }
                budget.charge_work(leaves.len())?;
                for (observed, expected) in leaves.iter().zip(&mut self.expected) {
                    if expected.is_none() {
                        *expected = observed.clone();
                    }
                }
            } else if leaves != self.expected {
                return Err(execution_identity_error_v1());
            }
            let retained = argument_product_v1(
                leaves.capacity(),
                std::mem::size_of::<Option<ExecutionCfgLeafV29>>(),
            )?;
            drop(physical);
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(scratch)
                    .and_then(|bytes| bytes.checked_sub(retained))
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )?;
            let mut returned = emission_vec_v1(values.len(), budget)?;
            budget.charge_work(values.len())?;
            returned.extend_from_slice(values);
            self.observations[slot].observation = Some(ExecutionIdentityReturnObservationV1 {
                source: block,
                target,
                values: returned,
                nominal: leaves,
            });
            Ok(())
        })
    }

    fn finish(
        &self,
        child: ProductionCallInstanceIdV1,
        callee: SemanticFunctionIdV1,
        output: &LoweredFunctionResultV1,
        instances: &ExecutionInstancesV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let original = instances
            .instance(child)
            .ok_or_else(execution_identity_error_v1)?;
        if original.function() != callee || output.source_call_instance != Some(child) {
            return Err(execution_identity_error_v1());
        }
        let exits = instances
            .exits(child)
            .ok_or_else(execution_identity_error_v1)?;
        budget.charge_work(argument_product_v1(exits.len(), 2)?)?;
        for exit in exits {
            if instances.block_reachable(child, exit.block).is_none() {
                return Err(execution_identity_error_v1());
            }
        }
        let returns = exits
            .iter()
            .filter(|exit| {
                instances.block_reachable(child, exit.block) == Some(true) && matches!(
                    exit.kind,
                    production_call_instances_v1::ProductionInstanceExitKindV1::Return { .. }
                )
            })
            .count();
        if self.observations.len() != returns || self.observations.is_empty() {
            return Err(execution_identity_error_v1());
        }
        let body = output
            .function
            .body
            .as_ref()
            .ok_or_else(execution_identity_error_v1)?;
        let mut bytes = argument_sum_v1(&[
            std::mem::size_of::<Option<ExecutionIdentityReturnWitnessV1>>(),
            argument_product_v1(
                self.expected.capacity(),
                std::mem::size_of::<Option<ExecutionCfgLeafV29>>(),
            )?,
            argument_product_v1(
                self.observations.capacity(),
                std::mem::size_of::<ExecutionIdentityReturnSlotV1>(),
            )?,
        ])?;
        let mut previous = None;
        for slot in &self.observations {
            let row = slot
                .observation
                .as_ref()
                .ok_or_else(execution_identity_error_v1)?;
            budget.charge_work(argument_sum_v1(&[
                row.nominal.len(),
                self.expected.len(),
                3,
            ])?)?;
            if previous.is_some_and(|block| block >= slot.exit.block)
                || row.source != slot.exit.block
                || row.nominal != self.expected
            {
                return Err(execution_identity_error_v1());
            }
            previous = Some(slot.exit.block);
            bytes = argument_sum_v1(&[
                bytes,
                argument_product_v1(row.values.capacity(), std::mem::size_of::<ValueId>())?,
                argument_product_v1(
                    row.nominal.capacity(),
                    std::mem::size_of::<Option<ExecutionCfgLeafV29>>(),
                )?,
            ])?;
        }
        for exit in exits {
            budget.charge_work(2)?;
            if !instances.block_reachable(child, exit.block).ok_or_else(execution_identity_error_v1)? {
                continue;
            }
            let production_call_instances_v1::ProductionInstanceExitKindV1::Return { local } =
                exit.kind
            else {
                continue;
            };
            charge_execution_cfg_lookup_v29(self.observations.len(), budget)?;
            let slot = self
                .observations
                .binary_search_by_key(&exit.block.index(), |slot| slot.exit.block.index())
                .map_err(|_| execution_identity_error_v1())?;
            if exit.instance != child || self.observations[slot].exit.local != local {
                return Err(execution_identity_error_v1());
            }
        }

        // Source mappings and physical blocks share the original RPO. Check
        // that single ordered join, including the authenticated prefix, rather
        // than rebuilding or repeatedly searching another physical block index.
        let owner = instances
            .instance(instances.root())
            .ok_or_else(execution_identity_error_v1)?
            .function();
        let (prefix, _) = invocation_checked_prefix_v1(original, owner, output, budget)?;
        let order = original.ssa().plan().reverse_postorder();
        budget.charge_work(argument_product_v1(order.len(), 3)?)?;
        for &block in order {
            if instances.block_reachable(child, SemanticBlockIdV1::from_index(block.get())).is_none() {
                return Err(execution_identity_error_v1());
            }
        }
        let active = |block: &&SsaBlockIdV1| instances.block_reachable(
            child, SemanticBlockIdV1::from_index(block.get()),
        ) == Some(true);
        let active_count = order.iter().filter(active).count();
        let first = output
            .blocks
            .first()
            .ok_or_else(execution_identity_error_v1)?;
        let base = first
            .kernel_ir_block
            .0
            .checked_sub(original.declaration().entry().index())
            .ok_or_else(execution_identity_error_v1)?;
        budget.charge_work(output.synthetic_operation_spans.len())?;
        let has_trap = output
            .synthetic_operation_spans
            .iter()
            .any(|span| span.rule == SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap);
        let source_end = argument_sum_v1(&[prefix, active_count])?;
        if output.blocks.len() != active_count
            || body.blocks.len() != argument_sum_v1(&[source_end, usize::from(has_trap)])?
        {
            return Err(execution_identity_error_v1());
        }
        let targets = body
            .blocks
            .get(prefix..source_end)
            .ok_or_else(execution_identity_error_v1)?;
        for ((source, mapping), block) in order.iter().filter(active).zip(&output.blocks).zip(targets) {
            budget.charge_work(6)?;
            let source = SemanticBlockIdV1::from_index(source.get());
            let target = base
                .checked_add(source.index())
                .map(BlockId)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            if mapping.semantic_function != callee
                || mapping.semantic_block != source
                || mapping.kernel_ir_block != target
                || block.id != target
            {
                return Err(execution_identity_error_v1());
            }
            if !matches!(
                original.declaration().blocks()[source.index() as usize]
                    .terminator()
                    .kind(),
                SemanticTerminatorKindV1::Return
            ) {
                continue;
            }
            charge_execution_cfg_lookup_v29(self.observations.len(), budget)?;
            let slot = self
                .observations
                .binary_search_by_key(&source.index(), |slot| slot.exit.block.index())
                .map_err(|_| execution_identity_error_v1())?;
            let row = self.observations[slot]
                .observation
                .as_ref()
                .ok_or_else(execution_identity_error_v1)?;
            budget.charge_work(row.values.len())?;
            if row.target != target
                || !matches!(&block.terminator, Some(Terminator::Return { values }) if *values == row.values)
            {
                return Err(execution_identity_error_v1());
            }
        }
        if has_trap {
            let trap = &body.blocks[source_end];
            let ordinal = u32::try_from(original.declaration().blocks().len())
                .map_err(|_| ArgumentResourceV1::Arithmetic)?;
            budget.charge_work(2)?;
            if trap.id
                != BlockId(
                    base.checked_add(ordinal)
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                )
                || !matches!(trap.terminator, Some(Terminator::Unreachable))
            {
                return Err(execution_identity_error_v1());
            }
        }
        Ok(bytes)
    }
}
