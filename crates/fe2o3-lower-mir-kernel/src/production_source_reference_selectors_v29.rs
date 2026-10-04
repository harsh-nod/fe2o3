// A selector names an original source use, not a numeric index or pointer permit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceReferenceSelectorValueV29 {
    Promoted(SsaValueV1),
    Retained { event: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceSelectorV29 {
    instance: ProductionCallInstanceIdV1,
    occurrence: usize,
    source: usize,
    projection: usize,
    local: SemanticLocalIdV1,
    value: SourceReferenceSelectorValueV29,
    array: SemanticTypeIdV1,
    element: SemanticTypeIdV1,
    length: u64,
    canonical: usize,
    // Observed only after the original implicit index read passes C2. This is
    // a scalar holder activation locator, never pointer freshness authority.
    retained_generation: Option<u32>,
}

type SourceReferenceSelectorSiteV29 = (usize, u32, Option<usize>, usize, usize);

fn source_reference_selector_place_v29(
    function: &SemanticFunctionDeclV1,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
) -> Option<&SemanticPlaceV1> {
    scoped_source_place_v29(function, site, role).or_else(|| {
        match scoped_source_operand_v29(function, site, role) {
            Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) => Some(place),
            _ => match scoped_source_statement_v29(function, site)? {
                SemanticStatementKindV1::Assign(assignment)
                    if role == ExecutionOperandV29::RvaluePlace =>
                {
                    match assignment.value().kind() {
                        SemanticRvalueKindV1::Length(place)
                        | SemanticRvalueKindV1::Discriminant(place) => Some(place),
                        _ => None,
                    }
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                    if role == ExecutionOperandV29::StatementPlace =>
                {
                    Some(place)
                }
                _ => None,
            },
        }
    })
}

fn source_reference_selector_site_v29(
    instance: ProductionCallInstanceIdV1,
    site: ExecutionSiteV29,
    source: &SemanticPlaceV1,
    projection: usize,
) -> SourceReferenceSelectorSiteV29 {
    let (block, statement) = scoped_memory_site_key_v29(site);
    (
        instance.index(),
        block,
        statement.map(|value| value as usize),
        source as *const SemanticPlaceV1 as usize,
        projection,
    )
}

impl SourceReferenceSelectorV29 {
    fn check<'a>(
        &self,
        instances: &'a ExecutionInstancesV29<'_>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<&'a SemanticPlaceV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(12)?;
        let instance = instances
            .instance(self.instance)
            .ok_or_else(execution_availability_error_v29)?;
        let occurrences = instances
            .occurrences(self.instance)
            .ok_or_else(execution_availability_error_v29)?;
        let event = occurrences
            .events()
            .get(self.occurrence)
            .ok_or_else(execution_availability_error_v29)?;
        let place = source_reference_selector_place_v29(
            instance.declaration(),
            event.site(),
            event.operand(),
        )
        .ok_or_else(execution_availability_error_v29)?;
        let projection =
            u32::try_from(self.projection).map_err(|_| ArgumentResourceV1::Arithmetic)?;
        if self.source != place as *const SemanticPlaceV1 as usize
            || !event.is_reachable()
            || instances.block_reachable(
                self.instance,
                SemanticBlockIdV1::from_index(scoped_memory_site_key_v29(event.site()).0),
            ) != Some(true)
            || event.role() != ExecutionEventV29::ProjectionIndexUse(projection)
            || event.event().variable().get() != self.local.index()
            || match self.value {
                SourceReferenceSelectorValueV29::Promoted(value) => {
                    !event.is_promoted()
                        || event.resolved()
                            != Some(SsaResolvedEventV1::Use {
                                variable: fe2o3_mir_model::SsaVariableIdV1::new(self.local.index()),
                                value,
                            })
                }
                SourceReferenceSelectorValueV29::Retained { event: original } => {
                    original != self.occurrence || event.is_promoted() || event.resolved().is_some()
                }
            }
            || place
                .projections()
                .get(self.projection)
                .map(|row| row.kind())
                != Some(SemanticProjectionKindV1::Index(self.local))
        {
            return Err(execution_availability_error_v29());
        }
        let array = if self.projection == 0 {
            instance
                .declaration()
                .locals()
                .get(place.local().index() as usize)
                .ok_or_else(execution_availability_error_v29)?
                .ty()
        } else {
            place.projections()[self.projection - 1].result_type()
        };
        let types = instances.owner().source_semantic().types();
        if array != self.array
            || !matches!(types.get(array.index() as usize).map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Array { element, length })
                    if *element == self.element && *length == self.length)
            || place.projections()[self.projection].result_type() != self.element
        {
            return Err(source_reference_error_v29(
                "source selector changed its original fixed-array extent",
            ));
        }
        Ok(place)
    }
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn retain_selector_generation(
        &mut self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        projection: usize,
        resolved: &SourceReferencePlaceV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<()>(budget)?;
        if self.storage_root.is_none() {
            return Ok(());
        }
        source_reference_emission_prepay_v29::<SourceReferenceSelectorV29>(budget)?;
        source_reference_emission_prepay_v29::<SourceReferenceSelectorSiteV29>(budget)?;
        source_reference_emission_prepay_v29::<Option<&usize>>(budget)?;
        source_reference_emission_prepay_v29::<Option<&SourceReferenceSelectorV29>>(budget)?;
        source_reference_emission_prepay_v29::<
            Option<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>,
        >(budget)?;
        source_reference_emission_prepay_v29::<Option<&SemanticTypeDeclV1>>(budget)?;
        source_reference_emission_prepay_v29::<Option<SemanticTypeIdV1>>(budget)?;
        charge_execution_cfg_lookup_v29(self.plan.selector_sites.len(), budget)?;
        let key = (
            site.instance.index(),
            site.block.index(),
            site.statement,
            source as *const SemanticPlaceV1 as usize,
            projection,
        );
        let Some(&index) = self.plan.selector_sites.get(&key) else {
            return Ok(());
        };
        budget.charge_work(12)?;
        let row = *self
            .plan
            .selectors
            .get(index)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let original = row.check(self.plan.instances, budget)?;
        if row.instance != site.instance
            || row.projection != projection
            || !std::ptr::eq(original, source)
            || resolved.instance != row.instance
            || resolved.local != row.local
            || resolved.loan.is_some()
            || resolved.shared_path
            || !resolved.projections.is_empty()
            || !resolved.traversed.is_empty()
        {
            return Err(source_reference_cfg_obligation_v29());
        }
        if !matches!(row.value, SourceReferenceSelectorValueV29::Retained { .. }) {
            return Ok(());
        }
        let local = self
            .plan
            .instances
            .instance(row.instance)
            .and_then(|instance| {
                instance
                    .declaration()
                    .locals()
                    .get(row.local.index() as usize)
            })
            .ok_or(ArgumentResourceV1::Accounting)?;
        let declaration = self
            .plan
            .instances
            .owner()
            .source_semantic()
            .types()
            .get(local.ty().index() as usize)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if self.plan.nodes.get(resolved.node).map(|node| node.ty) != Some(local.ty())
            || !matches!(
                declaration.shape(),
                SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
            )
        {
            return Err(source_reference_cfg_obligation_v29());
        }
        let generation = match row.retained_generation {
            Some(previous) if previous != resolved.generation => self.join_storage_epochs(
                row.instance,
                row.local,
                previous,
                resolved.generation,
                budget,
            )?,
            _ => resolved.generation,
        };
        self.plan.selectors[index].retained_generation = Some(generation);
        Ok(())
    }

    fn collect_storage_selectors(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.storage_root.is_none() {
            return Ok(());
        }
        let instances = self.plan.instances;
        for (ordinal, instance) in instances.instances().iter().enumerate() {
            let first_selector = self.plan.selectors.len();
            let first_descriptor = self.plan.descriptors.len();
            budget.charge_work(1)?;
            let id = instances
                .id_at(ordinal)
                .ok_or(ArgumentResourceV1::Accounting)?;
            match instances.instance_reachable(id) {
                Some(false) => continue,
                Some(true) => {}
                None => return Err(source_reference_cfg_obligation_v29()),
            }
            let occurrences = instances
                .occurrences(id)
                .ok_or(ArgumentResourceV1::Accounting)?;
            for (index, event) in occurrences.events().iter().enumerate() {
                budget.charge_work(2)?;
                let ExecutionEventV29::ProjectionIndexUse(projection) = event.role() else {
                    continue;
                };
                if !event.is_reachable() {
                    continue;
                }
                let block =
                    SemanticBlockIdV1::from_index(scoped_memory_site_key_v29(event.site()).0);
                if !instances
                    .block_reachable(id, block)
                    .ok_or_else(source_reference_cfg_obligation_v29)?
                {
                    continue;
                }
                let source = source_reference_selector_place_v29(
                    instance.declaration(),
                    event.site(),
                    event.operand(),
                )
                .ok_or_else(execution_availability_error_v29)?;
                let prefix = source
                    .projections()
                    .get(..projection as usize)
                    .ok_or_else(execution_availability_error_v29)?;
                budget.charge_work(prefix.len())?;
                // External slices retain their original descriptor and runtime
                // extent separately from fixed-array lengths.
                if prefix
                    .iter()
                    .any(|row| row.kind() == SemanticProjectionKindV1::Dereference)
                {
                    self.collect_storage_descriptor(
                        id,
                        index,
                        source,
                        projection as usize,
                        budget,
                    )?;
                    continue;
                }
                let array = prefix.last().map_or_else(
                    || instance.declaration().locals()[source.local().index() as usize].ty(),
                    |projection| projection.result_type(),
                );
                let Some(SemanticTypeShapeV1::Array { element, length }) = instances
                    .owner()
                    .source_semantic()
                    .types()
                    .get(array.index() as usize)
                    .map(SemanticTypeDeclV1::shape)
                else {
                    continue;
                };
                let (value, canonical) = match (event.is_promoted(), event.resolved()) {
                    (true, Some(SsaResolvedEventV1::Use { variable, value }))
                        if variable == event.event().variable() =>
                    {
                        let value_key = (id.index(), variable.get(), value);
                        charge_execution_cfg_lookup_v29(self.plan.selector_values.len(), budget)?;
                        let canonical = match self.plan.selector_values.get(&value_key) {
                            Some(&canonical) => canonical,
                            None => {
                                reserve_execution_cfg_map_entry_v29::<
                                    (usize, u32, SsaValueV1),
                                    usize,
                                >(
                                    self.plan.selector_values.len(), budget
                                )?;
                                let canonical = self.plan.selectors.len();
                                self.plan.selector_values.insert(value_key, canonical);
                                canonical
                            }
                        };
                        (SourceReferenceSelectorValueV29::Promoted(value), canonical)
                    }
                    (false, None) => {
                        budget.charge_work(2)?;
                        (
                            SourceReferenceSelectorValueV29::Retained { event: index },
                            self.plan.selectors.len(),
                        )
                    }
                    _ => return Err(execution_availability_error_v29()),
                };
                let selector = SourceReferenceSelectorV29 {
                    instance: id,
                    occurrence: index,
                    source: source as *const SemanticPlaceV1 as usize,
                    projection: projection as usize,
                    local: SemanticLocalIdV1::from_index(event.event().variable().get()),
                    value,
                    array,
                    element: *element,
                    length: *length,
                    canonical,
                    retained_generation: None,
                };
                selector.check(instances, budget)?;
                let key = source_reference_selector_site_v29(
                    id,
                    event.site(),
                    source,
                    projection as usize,
                );
                reserve_execution_cfg_map_entry_v29::<SourceReferenceSelectorSiteV29, usize>(
                    self.plan.selector_sites.len(),
                    budget,
                )?;
                let next = self.plan.selectors.len();
                emission_push_v1(&mut self.plan.selectors, selector, budget)?;
                if self.plan.selector_sites.insert(key, next).is_some() {
                    return Err(execution_availability_error_v29());
                }
            }
            if first_descriptor != self.plan.descriptors.len() {
                self.collect_descriptor_guards(id, budget)?;
            }
            if first_selector == self.plan.selectors.len() {
                continue;
            }
            for event in occurrences.events() {
                budget.charge_work(1)?;
                let block =
                    SemanticBlockIdV1::from_index(scoped_memory_site_key_v29(event.site()).0);
                if !instances
                    .block_reachable(id, block)
                    .ok_or_else(source_reference_cfg_obligation_v29)?
                {
                    continue;
                }
                if let Some(SsaResolvedEventV1::Define { variable, value }) = event.resolved() {
                    let (block, _) = scoped_memory_site_key_v29(event.site());
                    self.record_selector_redefinition(id, block, variable.get(), value, budget)?;
                }
            }
            for edge in occurrences.successors() {
                budget.charge_work(1)?;
                let block = SemanticBlockIdV1::from_index(edge.id().source().get());
                if !instances
                    .block_reachable(id, block)
                    .ok_or_else(source_reference_cfg_obligation_v29)?
                    || (edge.edge().role() == SemanticEdgeRoleV1::CallReturn
                        && !source_reference_call_returns_v29(instances, id, block, budget)?)
                {
                    continue;
                }
                for definition in instance
                    .ssa()
                    .plan()
                    .edge_definitions(edge.id())
                    .ok_or_else(execution_availability_error_v29)?
                {
                    self.record_selector_redefinition(
                        id,
                        edge.id().source().get(),
                        definition.variable().get(),
                        definition.value(),
                        budget,
                    )?;
                }
            }
            // Phi identities are re-evaluated on every visit to their block.
            for selector in first_selector..self.plan.selectors.len() {
                budget.charge_work(1)?;
                let row = self.plan.selectors[selector];
                if row.instance == id && row.canonical == selector {
                    match row.value {
                        SourceReferenceSelectorValueV29::Promoted(
                            value @ SsaValueV1::BlockArgument { block, variable },
                        ) => {
                            self.record_selector_redefinition(
                                id,
                                block.get(),
                                variable.get(),
                                value,
                                budget,
                            )?;
                        }
                        SourceReferenceSelectorValueV29::Retained { event } => {
                            let original = occurrences
                                .events()
                                .get(event)
                                .ok_or_else(execution_availability_error_v29)?;
                            let (block, _) = scoped_memory_site_key_v29(original.site());
                            self.record_selector_reset(id, block, selector, budget)?;
                        }
                        SourceReferenceSelectorValueV29::Promoted(_) => {}
                    }
                }
            }
        }
        Ok(())
    }

    fn record_selector_redefinition(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        block: u32,
        variable: u32,
        value: SsaValueV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        charge_execution_cfg_lookup_v29(self.plan.selector_values.len(), budget)?;
        let Some(&canonical) = self
            .plan
            .selector_values
            .get(&(instance.index(), variable, value))
        else {
            return Ok(());
        };
        self.record_selector_reset(instance, block, canonical, budget)
    }

    fn record_selector_reset(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        block: u32,
        canonical: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let key = (instance.index(), block);
        charge_execution_cfg_lookup_v29(self.plan.selector_redefinitions.len(), budget)?;
        if !self.plan.selector_redefinitions.contains_key(&key) {
            reserve_execution_cfg_map_entry_v29::<(usize, u32), Vec<usize>>(
                self.plan.selector_redefinitions.len(),
                budget,
            )?;
            self.plan.selector_redefinitions.insert(key, Vec::new());
        }
        let values = self
            .plan
            .selector_redefinitions
            .get_mut(&key)
            .ok_or(ArgumentResourceV1::Accounting)?;
        emission_push_v1(values, canonical, budget)
    }

    fn forget_live_selector_keys(
        &mut self,
        keys: &[usize],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.storage_root.is_none() {
            return Ok(());
        }
        for ordinal in 0..self.frames.len() {
            budget.charge_work(1)?;
            let Some(state) = self.frames[ordinal] else {
                continue;
            };
            for local in 0..self.plan.states[state].len() {
                budget.charge_work(1)?;
                let Some(index) = self.plan.states[state][local].storage else {
                    continue;
                };
                let snapshot = self
                    .storage_root
                    .as_ref()
                    .ok_or(ArgumentResourceV1::Accounting)?
                    .forget_snapshot_selectors(
                        self.storage_snapshot(index)?,
                        keys,
                        &self.plan,
                        budget,
                    )
                    .map_err(|error| self.storage_error(error))?;
                let retained = self.retain_storage_snapshot(snapshot, budget)?;
                self.plan.states[state][local].storage = Some(retained);
            }
        }
        Ok(())
    }

    fn enter_selector_block(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        block: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.plan.selectors.is_empty() {
            return Ok(());
        }
        charge_execution_cfg_lookup_v29(self.plan.selector_redefinitions.len(), budget)?;
        let Some(keys) = self
            .plan
            .selector_redefinitions
            .get(&(instance.index(), block))
        else {
            return Ok(());
        };
        let mut copied = source_reference_scratch_v29(keys.len(), budget)?;
        budget.charge_work(keys.len())?;
        copied.extend_from_slice(keys);
        self.forget_live_selector_keys(&copied, budget)?;
        source_reference_drop_scratch_v29(copied, budget)
    }

    fn leave_selector_invocation(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        result: SourceReferenceFunctionOutcomeV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceFunctionOutcomeV29, ProductionSemanticKirErrorV1> {
        if self.plan.selectors.is_empty() {
            return Ok(result);
        }
        let mut keys = source_reference_scratch_v29(0, budget)?;
        for (index, selector) in self.plan.selectors.iter().enumerate() {
            budget.charge_work(1)?;
            if selector.instance == instance && selector.canonical == index {
                emission_push_v1(&mut keys, index, budget)?;
            }
        }
        if keys.is_empty() {
            source_reference_drop_scratch_v29(keys, budget)?;
            return Ok(result);
        }
        self.forget_live_selector_keys(&keys, budget)?;
        let SourceReferenceFunctionOutcomeV29::Returned(value) = result else {
            source_reference_drop_scratch_v29(keys, budget)?;
            return Ok(result);
        };
        let mut node = self.plan.nodes[value];
        if let Some(mut storage) = node.storage {
            let snapshot = self
                .storage_root
                .as_ref()
                .ok_or(ArgumentResourceV1::Accounting)?
                .forget_snapshot_selectors(
                    self.storage_snapshot(storage.snapshot)?,
                    &keys,
                    &self.plan,
                    budget,
                )
                .map_err(|error| self.storage_error(error))?;
            storage.snapshot = self.retain_storage_snapshot(snapshot, budget)?;
            node.storage = Some(storage);
        }
        source_reference_drop_scratch_v29(keys, budget)?;
        if node == self.plan.nodes[value] {
            return Ok(result);
        }
        budget.charge_work(1)?;
        node.value_origin = Some(node.value_origin.unwrap_or(value));
        let next = self.plan.nodes.len();
        emission_push_v1(&mut self.plan.nodes, node, budget)?;
        Ok(SourceReferenceFunctionOutcomeV29::Returned(next))
    }
}

fn source_reference_drop_scratch_v29<T>(
    rows: Vec<T>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let owned = argument_sum_v1(&[
        std::mem::size_of::<Vec<T>>(),
        argument_product_v1(rows.capacity(), std::mem::size_of::<T>())?,
    ])?;
    drop(rows);
    budget.release_storage(owned).map_err(Into::into)
}

impl SourceReferencePlanV29<'_, '_> {
    fn selector_for_path(
        &self,
        source: Option<(SourceReferenceSiteV29, usize)>,
        projection: usize,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<SourceReferenceSelectorV29, ProductionSemanticKirErrorV1> {
        budget.source_reference_owner_v29(self)?;
        let (site, source) = source.ok_or_else(execution_availability_error_v29)?;
        let key = (
            site.instance.index(),
            site.block.index(),
            site.statement,
            source,
            projection,
        );
        charge_execution_cfg_lookup_v29(self.selector_sites.len(), budget)?;
        let &index = self
            .selector_sites
            .get(&key)
            .ok_or_else(execution_availability_error_v29)?;
        let row = *self
            .selectors
            .get(index)
            .ok_or(ArgumentResourceV1::Accounting)?;
        row.check(self.instances, budget)?;
        let canonical = self
            .selectors
            .get(row.canonical)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if row.instance != site.instance
            || row.source != source
            || row.projection != projection
            || canonical.instance != row.instance
            || canonical.local != row.local
            || canonical.value != row.value
            || canonical.canonical != row.canonical
        {
            return Err(execution_availability_error_v29());
        }
        Ok(row)
    }

    fn selector_at(
        &self,
        instance: ProductionCallInstanceIdV1,
        site: ExecutionSiteV29,
        source: &SemanticPlaceV1,
        projection: usize,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<(usize, SourceReferenceSelectorV29)>, ProductionSemanticKirErrorV1> {
        budget.source_reference_owner_v29(self)?;
        charge_execution_cfg_lookup_v29(self.selector_sites.len(), budget)?;
        let key = source_reference_selector_site_v29(instance, site, source, projection);
        let Some(&index) = self.selector_sites.get(&key) else {
            return Ok(None);
        };
        let row = *self
            .selectors
            .get(index)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let original = row.check(self.instances, budget)?;
        if row.instance != instance
            || row.projection != projection
            || !std::ptr::eq(source, original)
        {
            return Err(execution_availability_error_v29());
        }
        Ok(Some((index, row)))
    }
}
