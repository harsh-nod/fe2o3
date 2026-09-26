// This borrowed proof is tied to the actual destination cursor. It is never
// retained in output and cannot authorize an unrelated block or instance.
struct ExecutionIdentityDestinationV1<'a> {
    cfg: &'a ExecutionCfgV29<'a>,
    block: usize,
}

impl ExecutionIdentityPlanV1<'_, '_> {
    fn check_cursor(
        &self,
        cursor: &ExecutionAvailabilityV29<'_>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(9)?;
        let original = self
            .index
            .instances
            .instance(cursor.instance)
            .ok_or_else(execution_identity_error_v1)?;
        if self.ledger != budget.work_ledger_identity_v1()
            || self.source != cursor.source
            || budget.storage() < self.floor
            || !std::ptr::eq(original.declaration(), cursor.function)
            || !std::ptr::eq(original.ssa(), cursor.ssa)
            || cursor.identities.is_some_and(|(plan, instance)| {
                !std::ptr::eq(plan, self) || instance != cursor.instance
            })
            || cursor
                .references
                .is_some_and(|references| !std::ptr::eq(references.plan, self.references))
        {
            return Err(execution_identity_error_v1());
        }
        Ok(())
    }

    fn has_target(
        &self,
        instance: ProductionCallInstanceIdV1,
        block: u32,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        charge_execution_cfg_lookup_v29(self.targets.len(), budget)?;
        Ok(self
            .targets
            .binary_search(&(instance.index(), block))
            .is_ok())
    }

    fn destination<'a>(
        &self,
        instance: ProductionCallInstanceIdV1,
        cfg: &'a ExecutionCfgV29<'_>,
        block: SemanticBlockIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<ExecutionIdentityDestinationV1<'a>>, ProductionSemanticKirErrorV1> {
        if !self.has_target(instance, block.index(), budget)? {
            return Ok(None);
        }
        let range = cfg
            .ranges
            .get(block.index() as usize)
            .ok_or_else(execution_identity_error_v1)?;
        let entries = cfg
            .entries
            .get(range.clone())
            .ok_or_else(execution_identity_error_v1)?;
        let original = self
            .index
            .instances
            .instance(instance)
            .ok_or_else(execution_identity_error_v1)?;
        let types = self.index.instances.owner().source_semantic().types();
        if !std::ptr::eq(types, cfg.types) {
            return Err(execution_identity_error_v1());
        }
        let mut previous = None;
        let mut actual = 0usize;
        for entry in entries {
            budget.charge_work(2)?;
            if previous.is_some_and(|previous| previous >= entry.local) {
                return Err(execution_identity_error_v1());
            }
            previous = Some(entry.local);
            if entry.leaves.is_empty() {
                continue;
            }
            actual = argument_sum_v1(&[actual, 1])?;
            let header = self.index.header(
                instance,
                SsaBlockIdV1::new(block.index()),
                SemanticLocalIdV1::from_index(entry.local),
                budget,
            )?;
            if header.ty != entry.ty {
                return Err(execution_identity_error_v1());
            }
            // Access every solved channel, even when no physical parameter is
            // emitted for this nominal leaf.
            for component in 0..header.count {
                let _ = self.class(argument_sum_v1(&[header.first, component])?, budget)?;
            }
        }
        // A truncated cursor cannot manufacture an empty early-header proof.
        // Reconstruct the complete nominal roster from the original live-ins.
        let mut expected = 0usize;
        for variable in original
            .ssa()
            .plan()
            .live_in(SsaBlockIdV1::new(block.index()))
            .ok_or_else(execution_identity_error_v1)?
        {
            budget.charge_work(1)?;
            let ty = original.declaration().locals()[variable.get() as usize].ty();
            let leaves = execution_cfg_nominal_count_v29(types, ty, budget)?;
            if leaves == 0 {
                continue;
            }
            expected = argument_sum_v1(&[expected, 1])?;
            charge_execution_cfg_lookup_v29(entries.len(), budget)?;
            let entry = entries
                .binary_search_by_key(&variable.get(), |entry| entry.local)
                .ok()
                .and_then(|index| entries.get(index))
                .ok_or_else(execution_identity_error_v1)?;
            if entry.ty != ty || entry.leaves.len() != leaves || entry.leaves.end > cfg.leaves.len()
            {
                return Err(execution_identity_error_v1());
            }
        }
        if actual != expected || expected == 0 {
            return Err(execution_identity_error_v1());
        }
        Ok(Some(ExecutionIdentityDestinationV1 {
            cfg,
            block: block.index() as usize,
        }))
    }

    fn class(
        &self,
        source: usize,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        let mapped = *self
            .mapping
            .get(source)
            .ok_or_else(execution_identity_error_v1)?;
        self.classes
            .get(mapped)
            .copied()
            .filter(|class| *class != usize::MAX)
            .ok_or_else(execution_identity_error_v1)
    }

    fn incoming(
        &self,
        instance: ProductionCallInstanceIdV1,
        block: SemanticBlockIdV1,
        local: u32,
        ty: SemanticTypeIdV1,
        value: SsaValueV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !self.has_target(instance, block.index(), budget)? {
            return Ok(());
        }
        let local = SemanticLocalIdV1::from_index(local);
        let incoming = self.index.value(instance, value, local, budget)?;
        let header =
            self.index
                .header(instance, SsaBlockIdV1::new(block.index()), local, budget)?;
        if incoming.ty != ty || header.ty != ty || incoming.count != header.count {
            return Err(execution_identity_error_v1());
        }
        for component in 0..header.count {
            if self.class(argument_sum_v1(&[incoming.first, component])?, budget)?
                != self.class(argument_sum_v1(&[header.first, component])?, budget)?
            {
                return Err(execution_identity_error_v1());
            }
        }
        Ok(())
    }
}

impl<'a> ExecutionAvailabilityV29<'a> {
    fn install_retained_seeds_v1(
        &mut self,
        locals: &[(usize, SemanticValueBindingV1)],
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.retained_seeds.is_empty() {
            return Ok(());
        }
        let (identities, instance) = self.identities.ok_or_else(execution_identity_error_v1)?;
        identities.check_cursor(self, budget)?;
        if instance != self.instance {
            return Err(execution_identity_error_v1());
        }
        for (local, binding) in locals {
            budget.charge_work(2)?;
            let Some(row) = identities.index.retained_entry(
                instance,
                SemanticLocalIdV1::from_index(*local as u32),
                budget,
            )?
            else {
                continue;
            };
            let SemanticValueBindingV1::Execution(binding) = binding else {
                return Err(execution_identity_error_v1());
            };
            binding
                .check_type(
                    identities.index.instances.owner().source_semantic().types(),
                    row.selection.ty,
                )
                .map_err(|_| execution_identity_error_v1())?;
            let seed = self
                .retained_seeds
                .get_mut(*local)
                .ok_or_else(execution_identity_error_v1)?;
            if seed.is_some() {
                return Err(execution_identity_error_v1());
            }
            // Execution bindings have no dynamic allocation. This is the
            // actual prepared input, not a binding made from a solver label.
            *seed = Some(binding.clone());
        }
        charge_execution_cfg_lookup_v29(identities.index.retained.len(), budget)?;
        for (&(_, local), _) in identities
            .index
            .retained
            .range((instance.index(), 0)..=(instance.index(), u32::MAX))
        {
            budget.charge_work(2)?;
            if self
                .retained_seeds
                .get(local as usize)
                .and_then(Option::as_ref)
                .is_none()
            {
                return Err(execution_identity_error_v1());
            }
        }
        Ok(())
    }

    fn use_retained_place_v1(
        &mut self,
        site: ExecutionSiteV29,
        operand: ExecutionOperandV29,
        place: &SemanticPlaceV1,
        locals: &[Option<SemanticValueBindingV1>],
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let Some((identities, instance)) = self.identities else {
            return Ok(false);
        };
        if identities
            .index
            .retained_entry(instance, place.local(), budget)?
            .is_none()
        {
            return Ok(false);
        }
        identities.check_cursor(self, budget)?;
        if instance != self.instance {
            return Err(execution_identity_error_v1());
        }
        let original = execution_identity_retained_place_v1(self.function, site, operand, budget)?
            .ok_or_else(execution_identity_error_v1)?;
        if !std::ptr::eq(original, place) {
            return Err(execution_identity_error_v1());
        }
        let event = self
            .find_occurrence(site, operand, ExecutionEventV29::BaseUse, budget)?
            .ok_or_else(execution_identity_error_v1)?;
        let selected = identities
            .index
            .retained_use(instance, event, place.local(), budget)?;
        // Logical initialization was checked under the concrete C2 root for
        // this invocation. The closed effect certificate proves it unchanged.
        let _ = identities.class(selected.first, budget)?;
        budget.charge_work(3)?;
        let seed = self
            .retained_seeds
            .get(place.local().index() as usize)
            .and_then(Option::as_ref)
            .ok_or_else(execution_identity_error_v1)?;
        let Some(Some(SemanticValueBindingV1::Execution(current))) =
            locals.get(place.local().index() as usize)
        else {
            return Err(execution_identity_error_v1());
        };
        if seed != current {
            return Err(execution_identity_error_v1());
        }
        self.claim_events(&[event], budget)?;
        Ok(true)
    }

    fn with_identity_plan_v1(
        mut self,
        identities: Option<&'a ExecutionIdentityPlanV1<'a, 'a>>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        self.check_ledger(budget)?;
        if self.identities.is_some() {
            return Err(execution_identity_error_v1());
        }
        if let Some(identities) = identities {
            identities.check_cursor(&self, budget)?;
        }
        self.identities = identities.map(|plan| (plan, self.instance));
        Ok(self)
    }
}
