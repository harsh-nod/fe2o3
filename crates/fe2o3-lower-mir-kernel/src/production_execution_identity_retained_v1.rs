include!("production_execution_identity_retained_calls_v1.rs");

// This certificate preserves an actual immutable entry object. It never turns
// retained storage into an SSA value or supplies a nominal producer identity.
impl ExecutionIdentityPlanV1<'_, '_> {
    fn retained_slot_omission_v1(
        &self,
        instances: &ExecutionInstancesV29<'_>,
        references: &SourceReferencePlanV29<'_, '_>,
        instance: ProductionCallInstanceIdV1,
        local: u32,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(12)?;
        if !std::ptr::eq(self.index.instances, instances)
            || !std::ptr::eq(self.references, references)
            || !std::ptr::eq(references.instances, instances)
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
            || self.source.semantic != *instances.owner().source_semantic_sha256()
            || self.source.ssa != instances.owner().identity()
            || instances
                .instance(instances.root())
                .map(|row| row.function())
                != Some(self.source.root)
        {
            return Err(execution_identity_error_v1());
        }
        budget.source_reference_owner_v29(references)?;
        let Some(row) =
            self.index
                .retained_entry(instance, SemanticLocalIdV1::from_index(local), budget)?
        else {
            return Ok(false);
        };
        let declaration = instances
            .instance(instance)
            .and_then(|row| row.declaration().locals().get(local as usize))
            .ok_or_else(execution_identity_error_v1)?;
        if !declaration.role().is_entry_argument()
            || declaration.ty() != row.selection.ty
            || execution_cfg_nominal_kind_v29(
                instances.owner().source_semantic().types(),
                declaration.ty(),
            )? != Some(false)
        {
            return Err(execution_identity_error_v1());
        }
        let _ = self.class(row.selection.first, budget)?;
        Ok(true)
    }
}

impl ExecutionAvailabilityV29<'_> {
    fn retained_installed_slot_omission_v1(
        &self,
        local: u32,
        prepared: &PreparedInputTransportV1<'_, '_>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let Some((identities, instance)) = self.identities else {
            return Ok(false);
        };
        identities.check_cursor(self, budget)?;
        if identities
            .index
            .retained_entry(instance, SemanticLocalIdV1::from_index(local), budget)?
            .is_none()
        {
            return Ok(false);
        }
        prepared.check(self, self.function, self.ssa, budget)?;
        let references = self.references.ok_or_else(execution_identity_error_v1)?;
        if instance != self.instance
            || !identities.retained_slot_omission_v1(
                identities.index.instances,
                references.plan,
                instance,
                local,
                budget,
            )?
        {
            return Ok(false);
        }
        budget.charge_work(3)?;
        let seed = self
            .retained_seeds
            .get(local as usize)
            .and_then(Option::as_ref)
            .ok_or_else(execution_identity_error_v1)?;
        if !matches!(prepared.locals.get(local as usize),
            Some(Some(SemanticValueBindingV1::Execution(current))) if current == seed)
        {
            return Err(execution_identity_error_v1());
        }
        Ok(true)
    }
}

fn execution_identity_retained_seed_slots_v1(
    count: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<Option<SemanticExecutionBindingV29>>, ProductionSemanticKirErrorV1> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut seeds = emission_vec_v1(count, budget)?;
    budget.charge_work(count)?;
    seeds.resize(count, None);
    Ok(seeds)
}

fn execution_identity_retained_present_v1(
    instances: &ExecutionInstancesV29<'_>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let types = instances.owner().source_semantic().types();
    for ordinal in 0..instances.instances().len() {
        budget.charge_work(2)?;
        let instance = instances
            .id_at(ordinal)
            .ok_or_else(execution_identity_error_v1)?;
        budget.charge_work(1)?;
        if !instances.instance_reachable(instance).ok_or_else(execution_identity_error_v1)? {
            continue;
        }
        let row = instances
            .instance(instance)
            .ok_or_else(execution_identity_error_v1)?;
        let occurrences = instances
            .occurrences(instance)
            .ok_or_else(execution_identity_error_v1)?;
        for entry in occurrences.entry_definitions() {
            budget.charge_work(2)?;
            let ty = row.declaration().locals()[entry.variable().get() as usize].ty();
            if entry.value().is_none() && execution_cfg_nominal_kind_v29(types, ty)? == Some(false)
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn execution_identity_shared_type_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    Ok(execution_cfg_nominal_kind_v29(types, ty)? == Some(true)
        && matches!(types[ty.index() as usize].shape(), SemanticTypeShapeV1::Pointer(pointer)
            if pointer.mutability() == SemanticMutabilityV1::Immutable))
}

fn execution_identity_retained_place_v1<'a>(
    function: &'a SemanticFunctionDeclV1,
    site: ExecutionSiteV29,
    operand: ExecutionOperandV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<&'a SemanticPlaceV1>, ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    let ExecutionSiteV29::Statement { block, statement } = site else {
        return Ok(None);
    };
    if operand != ExecutionOperandV29::RvaluePlace {
        return Ok(None);
    }
    let statement = function
        .blocks()
        .get(block.get() as usize)
        .and_then(|block| block.statements().get(statement as usize))
        .ok_or_else(execution_identity_error_v1)?;
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Ok(None);
    };
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared,
        place,
    } = assignment.value().kind()
    else {
        return Ok(None);
    };
    Ok(place.projections().is_empty().then_some(place))
}

impl ExecutionIdentitySourceIndexV1<'_, '_> {
    fn add_retained_entries(
        &mut self,
        references: &SourceReferencePlanV29<'_, '_>,
        root: &source_storage_v29::SourceStorageRootV29<'_, '_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !self.retained.is_empty()
            || !references.retains_custody(self.instances, budget)
            || !root
                .custody_view()
                .retains_custody(self.instances, &references.failure, budget)
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let effects = ExecutionRetainedCallEffectsV1::derive(self, budget)?;
        let types = self.instances.owner().source_semantic().types();
        for ordinal in 0..self.instances.instances().len() {
            budget.charge_work(3)?;
            let instance = self
                .instances
                .id_at(ordinal)
                .ok_or_else(execution_identity_error_v1)?;
            budget.charge_work(1)?;
            if !self.instances.instance_reachable(instance).ok_or_else(execution_identity_error_v1)? {
                continue;
            }
            let row = self
                .instances
                .instance(instance)
                .ok_or_else(execution_identity_error_v1)?;
            let occurrences = self
                .instances
                .occurrences(instance)
                .ok_or_else(execution_identity_error_v1)?;
            // Root ABI nominal issuance is not an ordinary prepared argument.
            if self.instances.incoming(instance).is_none() {
                continue;
            }
            let mut any = false;
            for entry in occurrences.entry_definitions() {
                budget.charge_work(2)?;
                any |= entry.value().is_none()
                    && execution_cfg_nominal_kind_v29(
                        types,
                        row.declaration().locals()[entry.variable().get() as usize].ty(),
                    )? == Some(false);
            }
            if !any
                || execution_identity_channels_v1(types, row.declaration().abi().source_output_type(), budget)? != 0
                || !effects.accepts(self.instances, instance, budget)?
            {
                continue;
            }
            budget.reserve_storage(std::mem::size_of::<Vec<Option<usize>>>())?;
            let mut eligible = emission_vec_v1(row.declaration().locals().len(), budget)?;
            budget.charge_work(row.declaration().locals().len())?;
            eligible.resize(row.declaration().locals().len(), None);
            for (index, entry) in occurrences.entry_definitions().iter().enumerate() {
                budget.charge_work(2)?;
                let local = entry.variable().get() as usize;
                if entry.value().is_none()
                    && execution_cfg_nominal_kind_v29(
                        types,
                        row.declaration().locals()[local].ty(),
                    )? == Some(false)
                {
                    if eligible[local].replace(index).is_some() {
                        return Err(execution_identity_error_v1());
                    }
                }
            }
            for event in occurrences.events() {
                budget.charge_work(3)?;
                let local = event.event().variable().get() as usize;
                let block = match event.site() {
                    ExecutionSiteV29::Statement { block, .. }
                    | ExecutionSiteV29::Terminator { block } => block,
                };
                if !event.is_reachable() || eligible[local].is_none()
                    || !execution_identity_block_active_v1(self.instances, instance, block, budget)?
                {
                    continue;
                }
                let place = execution_identity_retained_place_v1(
                    row.declaration(),
                    event.site(),
                    event.operand(),
                    budget,
                )?;
                if event.is_promoted()
                    || event.resolved().is_some()
                    || event.role() != ExecutionEventV29::BaseUse
                    || place.is_none_or(|place| {
                        place.local().index() as usize != local
                            || place.ty() != row.declaration().locals()[local].ty()
                    })
                {
                    eligible[local] = None;
                }
            }
            for (local, entry) in eligible.iter().copied().enumerate() {
                budget.charge_work(2)?;
                let Some(entry) = entry else {
                    continue;
                };
                let local = SemanticLocalIdV1::from_index(local as u32);
                let parameter = self
                    .instances
                    .parameter_source(instance, local, budget)
                    .map_err(|error| match error {
                        production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(
                            error,
                        ) => error.into(),
                        _ => execution_identity_error_v1(),
                    })?;
                if parameter.ty != row.declaration().locals()[local.index() as usize].ty() {
                    return Err(execution_identity_error_v1());
                }
                let state = references
                    .entries
                    .get(instance.index())
                    .copied()
                    .flatten()
                    .ok_or_else(execution_identity_error_v1)?;
                let storage = references
                    .states
                    .get(state)
                    .and_then(|locals| locals.get(local.index() as usize))
                    .and_then(|local| local.storage)
                    .ok_or_else(execution_identity_error_v1)?;
                let snapshot = *references
                    .storage_snapshots
                    .get(storage)
                    .ok_or_else(execution_identity_error_v1)?;
                let initialized =
                    root.snapshot_initialized(snapshot, &[], budget)
                        .map_err(|recorded| {
                            if !references.failure.matches_recorded(&recorded) {
                                return ArgumentResourceV1::Accounting.into();
                            }
                            references
                                .failure
                                .first_error()
                                .unwrap_or_else(|| ArgumentResourceV1::Accounting.into())
                        })?;
                if !initialized {
                    return Err(execution_identity_error_v1());
                }
                let index = self.rows.len();
                self.push(
                    instance,
                    None,
                    local,
                    ExecutionIdentitySourceKindV1::RetainedEntry(entry),
                    budget,
                )?;
                if self.rows.len().checked_sub(index) != Some(1) {
                    return Err(execution_identity_error_v1());
                }
                reserve_execution_cfg_map_entry_v29::<(usize, u32), usize>(
                    self.retained.len(),
                    budget,
                )?;
                if self
                    .retained
                    .insert((instance.index(), local.index()), index)
                    .is_some()
                {
                    return Err(execution_identity_error_v1());
                }
            }
            let scratch = argument_sum_v1(&[
                std::mem::size_of::<Vec<Option<usize>>>(),
                argument_product_v1(eligible.capacity(), std::mem::size_of::<Option<usize>>())?,
            ])?;
            drop(eligible);
            budget.release_storage(scratch)?;
        }
        effects.discard(budget)?;
        Ok(())
    }

    fn retained_entry(
        &self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<&ExecutionIdentitySourceRowV1>, ProductionSemanticKirErrorV1> {
        charge_execution_cfg_lookup_v29(self.retained.len(), budget)?;
        let Some(&index) = self.retained.get(&(instance.index(), local.index())) else {
            return Ok(None);
        };
        let row = self
            .rows
            .get(index)
            .ok_or_else(execution_identity_error_v1)?;
        let ExecutionIdentitySourceKindV1::RetainedEntry(ordinal) = row.kind else {
            return Err(execution_identity_error_v1());
        };
        let occurrences = self
            .instances
            .occurrences(instance)
            .ok_or_else(execution_identity_error_v1)?;
        budget.charge_work(3)?;
        let original_type = self
            .instances
            .instance(instance)
            .and_then(|instance| instance.declaration().locals().get(local.index() as usize))
            .ok_or_else(execution_identity_error_v1)?
            .ty();
        let entry = occurrences
            .entry_definitions()
            .get(ordinal)
            .ok_or_else(execution_identity_error_v1)?;
        if row.instance != instance
            || row.local != local
            || row.value.is_some()
            || entry.value().is_some()
            || entry.variable().get() != local.index()
            || row.selection.ty != original_type
            || row.selection.count != 1
        {
            return Err(execution_identity_error_v1());
        }
        Ok(Some(row))
    }

    fn retained_use(
        &self,
        instance: ProductionCallInstanceIdV1,
        index: usize,
        local: SemanticLocalIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<ExecutionIdentitySelectionV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let row = self
            .retained_entry(instance, local, budget)?
            .ok_or_else(execution_identity_error_v1)?;
        let original = self
            .instances
            .instance(instance)
            .ok_or_else(execution_identity_error_v1)?;
        let occurrences = self
            .instances
            .occurrences(instance)
            .ok_or_else(execution_identity_error_v1)?;
        let event = occurrences
            .events()
            .get(index)
            .ok_or_else(execution_identity_error_v1)?;
        let place = execution_identity_retained_place_v1(
            original.declaration(),
            event.site(),
            event.operand(),
            budget,
        )?
        .ok_or_else(execution_identity_error_v1)?;
        let block = match event.site() {
            ExecutionSiteV29::Statement { block, .. }
            | ExecutionSiteV29::Terminator { block } => block,
        };
        if !event.is_reachable()
            || !execution_identity_block_active_v1(self.instances, instance, block, budget)?
            || event.is_promoted()
            || event.resolved().is_some()
            || event.role() != ExecutionEventV29::BaseUse
            || event.event().variable().get() != local.index()
            || place.local() != local
            || place.ty() != row.selection.ty
        {
            return Err(execution_identity_error_v1());
        }
        Ok(row.selection)
    }
}
