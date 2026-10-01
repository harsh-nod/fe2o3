use fe2o3_pliron::{
    ProductionSemanticSsaEventRoleV1 as ExecutionEventV29,
    ProductionSemanticSsaFunctionOccurrencesV1,
    ProductionSemanticSsaOccurrenceSiteV1 as ExecutionSiteV29,
    ProductionSemanticSsaOperandRoleV1 as ExecutionOperandV29,
};
use production_call_instances_v1::ProductionCallInstancePlanV1 as ExecutionInstancesV29;

include!("production_execution_control_v29.rs");

// This cursor consumes the existing owner's SSA resolutions. It neither issues
// execution roles nor proves that a borrow stays within its provider's scope.
struct ExecutionAvailabilityV29<'a> {
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    instance: ProductionCallInstanceIdV1,
    source: ExecutionCallSourceV29,
    function_id: SemanticFunctionIdV1,
    function: &'a SemanticFunctionDeclV1,
    ssa: &'a ProductionSemanticSsaFunctionPlanV1,
    occurrences: ProductionSemanticSsaFunctionOccurrencesV1<'a>,
    control: ExecutionSourceControlV29<'a>,
    index: Vec<UnitLocalSourceIndexV1>,
    claimed: Vec<bool>,
    current: Vec<Option<SsaValueV1>>,
    seen: Vec<bool>,
    visited: Vec<bool>,
    block: Option<SsaBlockIdV1>,
    cfg: ExecutionCfgV29<'a>,
    events: ExecutionEventsV29,
    parameters: Option<PreparedExecutionParametersV29<'a>>,
    invocation_inputs: Option<Vec<InvocationInputRowV1>>,
    retained_seeds: Vec<Option<SemanticExecutionBindingV29>>,
    references: Option<&'a SourceReferenceEmissionV29<'a, 'a>>,
    identities: Option<(
        &'a ExecutionIdentityPlanV1<'a, 'a>,
        ProductionCallInstanceIdV1,
    )>,
    #[cfg(test)]
    entry_seeds: Vec<(u32, SemanticValueBindingV1)>,
    #[cfg(test)]
    skipped_event: Option<usize>,
}

// This cursor owns prepaid buffers. Requiring whole-value destruction prevents
// an ordinary partial move from exporting a buffer before the owner refunds it.
// Explicit transfers still require their own metering; Drop never refunds.
impl Drop for ExecutionAvailabilityV29<'_> {
    fn drop(&mut self) {}
}

fn execution_availability_error_v29() -> ProductionSemanticKirErrorV1 {
    unsupported(
        0,
        None,
        None,
        "execution availability differs from its source SSA instance",
    )
}

// Audited private consumers drop the cursor; they must not extract its vectors.
// Only its fixed scratch reservation is released. Escaping output rows stay paid.
fn with_execution_availability_v29<R>(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl for<'a> FnOnce(
        ExecutionAvailabilityV29<'a>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let construction = catch_unwind(AssertUnwindSafe(|| {
        ExecutionAvailabilityV29::new(instances, instance, budget)
    }));
    let storage = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let result = match construction {
        Ok(Ok(cursor)) => catch_unwind(AssertUnwindSafe(|| consume(cursor, budget))),
        Ok(Err(error)) => Ok(Err(error)),
        Err(payload) => Err(payload),
    };
    let release = if budget.work_ledger_identity_v1() == ledger {
        budget.release_storage(storage)
    } else {
        Err(ArgumentResourceV1::Accounting)
    };
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

fn source_reference_availability_headers_v29<R>() -> Result<usize, ArgumentResourceV1> {
    use std::mem::size_of;
    type Payload = Box<dyn std::any::Any + Send>;
    argument_sum_v1(&[
        source_reference_emission_headers_v29::<ExecutionAvailabilityV29<'_>>()?,
        argument_product_v1(2, size_of::<Result<R, ProductionSemanticKirErrorV1>>())?,
        size_of::<Result<Result<R, ProductionSemanticKirErrorV1>, Payload>>(),
        size_of::<[Option<Payload>; 2]>(),
        size_of::<Result<(), Payload>>(),
        size_of::<Option<usize>>(),
        size_of::<Option<ArgumentResourceV1>>(),
        size_of::<Option<CompletedExecutionAvailabilityV1<'static>>>(),
        size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        argument_product_v1(4, size_of::<usize>())?,
    ])
}

// This private owner scope turns panics into refusals and destroys rejected
// outputs before recovery, including destructor panics and their payloads.
fn with_source_reference_availability_v29<'a, R>(
    instances: &ExecutionInstancesV29<'a>,
    instance: ProductionCallInstanceIdV1,
    references: Option<&'a SourceReferenceEmissionV29<'a, 'a>>,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl for<'cursor> FnOnce(
        ExecutionAvailabilityV29<'cursor>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    with_source_reference_availability_and_identity_v1(
        instances, instance, references, None, budget, consume,
    )
}

fn with_source_reference_availability_and_identity_v1<'a, R>(
    instances: &ExecutionInstancesV29<'a>,
    instance: ProductionCallInstanceIdV1,
    references: Option<&'a SourceReferenceEmissionV29<'a, 'a>>,
    identities: Option<&'a ExecutionIdentityPlanV1<'a, 'a>>,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl for<'cursor> FnOnce(
        ExecutionAvailabilityV29<'cursor>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    if let Some(references) = references {
        references.check(budget)?;
    }
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const ArgumentBudgetV1<'_> as usize;
    let headers = source_reference_availability_headers_v29::<R>()
        .map_err(ProductionSemanticKirErrorV1::from)
        .inspect_err(|error| {
            if let Some(references) = references {
                source_reference_record_failure_v29(references.plan, error);
            }
        })?;
    budget
        .reserve_storage(headers)
        .map_err(ProductionSemanticKirErrorV1::from)
        .inspect_err(|error| {
            if let Some(references) = references {
                source_reference_record_failure_v29(references.plan, error);
            }
        })?;
    let mut retained = None;
    let mut lease = None;
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| {
        // Prepay the extra wrapper result and outer-header cleanup checks.
        budget.charge_work(
            if references.is_some_and(|references| references.plan.storage_root.is_some()) {
                4 + 4 + 4 + 4
            } else {
                4 + 4
            },
        )?;
        let owned =
            OwnedExecutionAvailabilityV1::new(instances, instance, references, identities, budget)
                .inspect_err(|error| {
                    if let Some(references) = references {
                        source_reference_record_failure_v29(references.plan, error);
                    }
                })?;
        retained = Some(budget.storage());
        let (completed, outcome) = owned.consume(budget, consume);
        lease = Some(completed);
        match outcome {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    })) {
        Ok(result) => result,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(source_reference_error_v29(
                "source reference availability construction or callback panicked",
            ))
        }
    };
    let required = retained.unwrap_or(budget.storage());
    let same = budget.work_ledger_identity_v1() == ledger
        && budget as *const ArgumentBudgetV1<'_> as usize == slot;
    let lost = !same
        || budget.storage() < required
        || lease
            .as_ref()
            .is_some_and(|lease| !lease.permits_release(budget));
    if lost {
        if let Some(root) = references.and_then(|references| references.plan.storage_root.as_ref())
        {
            root.deny_active_root_refund();
        }
    }
    let first_failure = references.and_then(|references| references.plan.failure.get());
    if first_failure.is_some() || lost {
        if first_failure.is_some() || result.is_ok() {
            let rejected = std::mem::replace(
                &mut result,
                Err(first_failure
                    .unwrap_or(ArgumentResourceV1::Accounting)
                    .into()),
            );
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
                payloads[1] = Some(payload);
            }
        }
    }
    let destructor_panicked = source_reference_discard_v29(payloads);
    if destructor_panicked && result.is_ok() {
        result = Err(source_reference_error_v29(
            "source reference availability destructor panicked",
        ));
    }
    let released = match lease {
        Some(lease) => lease.release(budget),
        None if same && !lost => Ok(()),
        None => Err(ArgumentResourceV1::Accounting.into()),
    };
    if let Err(error) = released {
        if result.is_ok() {
            result = Err(error);
        }
    } else if same && !lost {
        let allowed = budget.permits_prepared_input_refund_v1(
            references.map(|references| references.plan),
            slot,
            ledger,
            argument_sum_v1(&[floor, headers])?,
            headers,
        );
        if !allowed {
            if let Some(root) =
                references.and_then(|references| references.plan.storage_root.as_ref())
            {
                root.deny_active_root_refund();
            }
            if result.is_ok() {
                result = Err(ArgumentResourceV1::Accounting.into());
            }
        } else if let Err(error) = budget.release_storage(headers) {
            if result.is_ok() {
                result = Err(error.into());
            }
        }
    }
    result
}

impl<'a> ExecutionAvailabilityV29<'a> {
    fn new(
        instances: &ExecutionInstancesV29<'a>,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::new_with_references(instances, instance, None, budget)
    }

    fn new_with_references(
        instances: &ExecutionInstancesV29<'a>,
        instance: ProductionCallInstanceIdV1,
        references: Option<&'a SourceReferenceEmissionV29<'a, 'a>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::new_with_identity(instances, instance, references, None, budget)
    }

    fn new_with_identity(
        instances: &ExecutionInstancesV29<'a>,
        instance: ProductionCallInstanceIdV1,
        references: Option<&'a SourceReferenceEmissionV29<'a, 'a>>,
        identities: Option<&'a ExecutionIdentityPlanV1<'a, 'a>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        if let Some(references) = references {
            references.plan.check_owner(instances, budget)?;
        }
        budget.charge_work(4)?;
        let row = instances
            .instance(instance)
            .ok_or_else(execution_availability_error_v29)?;
        let occurrences = instances
            .occurrences(instance)
            .ok_or_else(execution_availability_error_v29)?;
        let control = ExecutionSourceControlV29::new(instances, instance, budget)?;
        let source = ExecutionCallSourceV29::from_instances(instances, budget)?;
        control.check_source(source, row.declaration(), row.ssa(), instance, budget)?;
        let mut index = unit_local_vec_v1(occurrences.events().len(), budget)?;
        budget.charge_work(occurrences.events().len())?;
        for (ordinal, event) in occurrences.events().iter().enumerate() {
            index.push(UnitLocalSourceIndexV1 {
                key: unit_local_source_key_v1(event.site(), event.operand(), Some(event.role())),
                index: ordinal,
            });
        }
        unit_local_source_sort_v1(&mut index, budget)?;
        let mut claimed = unit_local_vec_v1(index.len(), budget)?;
        let mut current = unit_local_vec_v1(row.declaration().locals().len(), budget)?;
        let mut seen = unit_local_vec_v1(row.declaration().locals().len(), budget)?;
        let mut visited = unit_local_vec_v1(row.declaration().blocks().len(), budget)?;
        budget.charge_work(index.len())?;
        budget.charge_work(argument_product_v1(row.declaration().locals().len(), 2)?)?;
        budget.charge_work(row.declaration().blocks().len())?;
        claimed.resize(index.len(), false);
        current.resize(row.declaration().locals().len(), None);
        seen.resize(current.len(), false);
        visited.resize(row.declaration().blocks().len(), false);
        let cfg = ExecutionCfgV29::new_with_references(
            instances.owner().source_semantic().types(),
            row.declaration(),
            row.ssa(),
            &occurrences,
            references.map(|references| (references, instance)),
            Some(&control),
            budget,
        )?;
        let events = ExecutionEventsV29::new_with_identity(
            &occurrences,
            row.declaration(),
            &cfg.nominal_locals,
            &cfg.reference_locals,
            references.map(|references| (references, instance)),
            identities.map(|identities| (identities, instance)),
            Some(&control),
            budget,
        )?;
        let retained_count = if let Some(identities) = identities {
            charge_execution_cfg_lookup_v29(identities.index.retained.len(), budget)?;
            if identities
                .index
                .retained
                .range((instance.index(), 0)..=(instance.index(), u32::MAX))
                .next()
                .is_some()
            {
                row.declaration().locals().len()
            } else {
                0
            }
        } else {
            0
        };
        let retained_seeds = execution_identity_retained_seed_slots_v1(retained_count, budget)?;
        let cursor = Self {
            ledger: budget.work_ledger_identity_v1(),
            instance,
            source,
            function_id: row.function(),
            function: row.declaration(),
            ssa: row.ssa(),
            occurrences,
            control,
            index,
            claimed,
            current,
            seen,
            visited,
            block: None,
            cfg,
            events,
            parameters: None,
            invocation_inputs: None,
            retained_seeds,
            references,
            identities: identities.map(|plan| (plan, instance)),
            #[cfg(test)]
            entry_seeds: Vec::new(),
            #[cfg(test)]
            skipped_event: None,
        };
        if let Some(identities) = identities {
            if !std::ptr::eq(identities.index.instances, instances) {
                return Err(execution_identity_error_v1());
            }
            identities.check_cursor(&cursor, budget)?;
        }
        Ok(cursor)
    }

    fn check_source(
        &self,
        function: &SemanticFunctionDeclV1,
        ssa: &ProductionSemanticSsaFunctionPlanV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if std::ptr::eq(self.function, function) && std::ptr::eq(self.ssa, ssa) {
            Ok(())
        } else {
            Err(execution_availability_error_v29())
        }
    }

    fn check_ledger(
        &self,
        budget: &dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.ledger == budget.work_ledger_identity_v1() {
            Ok(())
        } else {
            Err(execution_availability_error_v29())
        }
    }

    fn begin_block(
        &mut self,
        block: SemanticBlockIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_ledger(budget)?;
        let block = SsaBlockIdV1::new(block.index());
        self.events.complete(budget)?;
        budget.charge_work(argument_product_v1(self.current.len(), 2)?)?;
        if self.block.is_some()
            || !self
                .source_block_reachable_v29(SemanticBlockIdV1::from_index(block.get()), budget)?
            || *self
                .visited
                .get(block.get() as usize)
                .ok_or_else(execution_availability_error_v29)?
        {
            return Err(execution_availability_error_v29());
        }
        self.current.fill(None);
        self.seen.fill(false);
        if block.get() == self.function.entry().index() {
            for definition in self.ssa.plan().entry_definitions() {
                budget.charge_work(1)?;
                self.current[definition.variable().get() as usize] = Some(definition.value());
            }
        }
        // First resolved use/kill selects the incoming definition. A first
        // definition needs no incoming value. This is a view, not a CFG replay.
        for (_, event) in self
            .ssa
            .plan()
            .resolved_events(block)
            .ok_or_else(execution_availability_error_v29)?
        {
            budget.charge_work(2)?;
            let (variable, value) = match *event {
                SsaResolvedEventV1::Use { variable, value } => (variable, Some(value)),
                SsaResolvedEventV1::Kill { variable, previous } => (variable, previous),
                SsaResolvedEventV1::Define { variable, .. } => (variable, None),
            };
            let local = variable.get() as usize;
            if !self.seen[local] {
                self.seen[local] = true;
                self.current[local] = value;
            }
        }
        self.visited[block.get() as usize] = true;
        self.block = Some(block);
        self.events.pending = self.events.blocks[block.get() as usize].clone();
        Ok(())
    }

    fn event(
        &self,
        site: ExecutionSiteV29,
        operand: ExecutionOperandV29,
        role: ExecutionEventV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        self.find_event(site, operand, role, budget)?
            .ok_or_else(execution_availability_error_v29)
    }

    fn find_event(
        &self,
        site: ExecutionSiteV29,
        operand: ExecutionOperandV29,
        role: ExecutionEventV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        let index = self.find_occurrence(site, operand, role, budget)?;
        if let Some(index) = index {
            let event = &self.occurrences.events()[index];
            if !event.is_promoted() || event.resolved().is_none() {
                return Err(execution_availability_error_v29());
            }
        }
        Ok(index)
    }

    // A retained source occurrence is not an SSA resolution. Its consumer
    // must separately validate the source storage or use find_event instead.
    fn find_occurrence(
        &self,
        site: ExecutionSiteV29,
        operand: ExecutionOperandV29,
        role: ExecutionEventV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        self.check_ledger(budget)?;
        let block = match site {
            ExecutionSiteV29::Statement { block, .. } | ExecutionSiteV29::Terminator { block } => {
                block
            }
        };
        if self.block != Some(block) {
            return Err(execution_availability_error_v29());
        }
        let key = unit_local_source_key_v1(site, operand, Some(role));
        let (mut left, mut right) = (0, self.index.len());
        while left < right {
            budget.charge_work(8)?;
            let middle = left + (right - left) / 2;
            match self.index[middle].key.cmp(&key) {
                std::cmp::Ordering::Less => left = middle + 1,
                std::cmp::Ordering::Greater => right = middle,
                std::cmp::Ordering::Equal => {
                    let index = self.index[middle].index;
                    let event = &self.occurrences.events()[index];
                    if self.claimed[index] || !event.is_reachable() {
                        return Err(execution_availability_error_v29());
                    }
                    return Ok(Some(index));
                }
            }
        }
        Ok(None)
    }

    fn use_place(
        &mut self,
        site: ExecutionSiteV29,
        operand: ExecutionOperandV29,
        place: &SemanticPlaceV1,
        moved: bool,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
        self.check_ledger(budget)?;
        budget.charge_work(3)?;
        let index = self.event(site, operand, ExecutionEventV29::BaseUse, budget)?;
        let Some(SsaResolvedEventV1::Use { variable, value }) =
            self.occurrences.events()[index].resolved()
        else {
            return Err(execution_availability_error_v29());
        };
        let local = place.local().index() as usize;
        if variable.get() != place.local().index() || self.current.get(local) != Some(&Some(value))
        {
            return Err(execution_availability_error_v29());
        }
        let kill = if moved && place.projections().is_empty() {
            let kill = self.event(site, operand, ExecutionEventV29::MoveKill, budget)?;
            if self.occurrences.events()[kill].resolved()
                != Some(SsaResolvedEventV1::Kill {
                    variable,
                    previous: Some(value),
                })
            {
                return Err(execution_availability_error_v29());
            }
            Some(kill)
        } else {
            None
        };
        if let Some(kill) = kill {
            self.claim_events(&[index, kill], budget)?;
            self.current[local] = None;
        } else {
            self.claim_events(&[index], budget)?;
        }
        Ok(value)
    }

    fn check_claimed_original_use_v29(
        &self,
        site: ExecutionSiteV29,
        operand: ExecutionOperandV29,
        place: &SemanticPlaceV1,
        definition: SsaValueV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_ledger(budget)?;
        budget.charge_work(5)?;
        let block = match site {
            ExecutionSiteV29::Statement { block, .. } | ExecutionSiteV29::Terminator { block } => {
                block
            }
        };
        if self.block != Some(block)
            || !scoped_object_original_place_v29(self.function, site, operand)
                .is_some_and(|original| std::ptr::eq(original, place))
        {
            return Err(execution_availability_error_v29());
        }
        let key = unit_local_source_key_v1(site, operand, Some(ExecutionEventV29::BaseUse));
        let (mut left, mut right) = (0, self.index.len());
        while left < right {
            budget.charge_work(8)?;
            let middle = left + (right - left) / 2;
            match self.index[middle].key.cmp(&key) {
                std::cmp::Ordering::Less => left = middle + 1,
                std::cmp::Ordering::Greater => right = middle,
                std::cmp::Ordering::Equal => {
                    let index = self.index[middle].index;
                    let event = &self.occurrences.events()[index];
                    if self.claimed[index]
                        && event.is_promoted()
                        && event.is_reachable()
                        && event.resolved()
                            == Some(SsaResolvedEventV1::Use {
                                variable: fe2o3_mir_model::SsaVariableIdV1::new(
                                    place.local().index(),
                                ),
                                value: definition,
                            })
                    {
                        return Ok(());
                    }
                    return Err(execution_availability_error_v29());
                }
            }
        }
        Err(execution_availability_error_v29())
    }

    fn check_claimed_original_operand_v46(
        &self,
        site: ExecutionSiteV29,
        operand: ExecutionOperandV29,
        place: &SemanticPlaceV1,
        definition: SsaValueV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_claimed_original_use_v29(site, operand, place, definition, budget)?;
        budget.charge_work(7)?;
        let moved = match scoped_source_operand_v29(self.function, site, operand) {
            Some(SemanticOperandV1::Copy(original)) if std::ptr::eq(original, place) => false,
            Some(SemanticOperandV1::Move(original)) if std::ptr::eq(original, place) => true,
            _ => return Err(execution_availability_error_v29()),
        };
        let local = place.local().index() as usize;
        if !moved || !place.projections().is_empty() {
            return if self.current.get(local) == Some(&Some(definition)) {
                Ok(())
            } else {
                Err(execution_availability_error_v29())
            };
        }
        // The source Move was consumed before the payload was emitted. Reopen
        // its exact claimed kill instead of requiring the consumed SSA value.
        if self.current.get(local) != Some(&None) {
            return Err(execution_availability_error_v29());
        }
        let key = unit_local_source_key_v1(site, operand, Some(ExecutionEventV29::MoveKill));
        let (mut left, mut right) = (0, self.index.len());
        while left < right {
            budget.charge_work(8)?;
            let middle = left + (right - left) / 2;
            match self.index[middle].key.cmp(&key) {
                std::cmp::Ordering::Less => left = middle + 1,
                std::cmp::Ordering::Greater => right = middle,
                std::cmp::Ordering::Equal => {
                    let index = self.index[middle].index;
                    let event = &self.occurrences.events()[index];
                    if self.claimed[index]
                        && event.is_promoted()
                        && event.is_reachable()
                        && event.resolved()
                            == Some(SsaResolvedEventV1::Kill {
                                variable: fe2o3_mir_model::SsaVariableIdV1::new(
                                    place.local().index(),
                                ),
                                previous: Some(definition),
                            })
                    {
                        return Ok(());
                    }
                    return Err(execution_availability_error_v29());
                }
            }
        }
        Err(execution_availability_error_v29())
    }

    fn define(
        &mut self,
        site: ExecutionSiteV29,
        local: SemanticLocalIdV1,
        value: SsaValueV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let index = match self.find_event(
            site,
            ExecutionOperandV29::Destination,
            ExecutionEventV29::DestinationDefine,
            budget,
        )? {
            Some(index) => index,
            None => self.event(
                site,
                ExecutionOperandV29::ElidedBorrowDestination,
                ExecutionEventV29::DestinationDefine,
                budget,
            )?,
        };
        if self.occurrences.events()[index].resolved()
            != Some(SsaResolvedEventV1::Define {
                variable: fe2o3_mir_model::SsaVariableIdV1::new(local.index()),
                value,
            })
        {
            return Err(execution_availability_error_v29());
        }
        self.claim_events(&[index], budget)?;
        self.current[local.index() as usize] = Some(value);
        Ok(())
    }

    fn storage_kill(
        &mut self,
        site: ExecutionSiteV29,
        operand: ExecutionOperandV29,
        local: SemanticLocalIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let index = self.event(site, operand, ExecutionEventV29::StorageKill, budget)?;
        let Some(SsaResolvedEventV1::Kill { variable, previous }) =
            self.occurrences.events()[index].resolved()
        else {
            return Err(execution_availability_error_v29());
        };
        if variable.get() != local.index() || self.current[local.index() as usize] != previous {
            return Err(execution_availability_error_v29());
        }
        self.claim_events(&[index], budget)?;
        self.current[local.index() as usize] = None;
        Ok(())
    }

    fn retained_operand(
        &self,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
    ) -> Option<&SemanticOperandV1> {
        scoped_source_operand_v29(self.function, site, role)
    }

    fn consume_failure_tail(
        &mut self,
        block: SemanticBlockIdV1,
        message: &SemanticAssertMessageV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_ledger(budget)?;
        budget.charge_work(6)?;
        let id = SsaBlockIdV1::new(block.index());
        let Some(SemanticTerminatorKindV1::Assert {
            message: original,
            unwind,
            ..
        }) = self
            .function
            .blocks()
            .get(block.index() as usize)
            .map(|block| block.terminator().kind())
        else {
            return Err(execution_availability_error_v29());
        };
        if self.block != Some(id)
            || !std::ptr::eq(original, message)
            || matches!(unwind, SemanticUnwindActionV1::Cleanup(_))
        {
            return Err(execution_availability_error_v29());
        }
        let boundary = self
            .occurrences
            .terminal_failure_start(id)
            .ok_or_else(execution_availability_error_v29)?;
        for index in 0..2 {
            budget.charge_work(3)?;
            if let Some(operand) = execution_assert_operand_v29(message, index) {
                let ty = match operand {
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place.ty(),
                    SemanticOperandV1::Constant(constant) => constant.ty(),
                };
                if !matches!(
                    self.cfg
                        .types
                        .get(ty.index() as usize)
                        .map(SemanticTypeDeclV1::shape),
                    Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
                ) {
                    return Err(execution_availability_error_v29());
                }
            }
        }
        let events = self.occurrences.events();
        budget.charge_work(argument_product_v1(
            16,
            events.len().checked_ilog2().unwrap_or(0) as usize + 2,
        )?)?;
        let first = events.partition_point(|event| {
            let event_block = execution_event_block_v29(event.site());
            event_block < id || (event_block == id && (event.ordinal() as usize) < boundary)
        });
        let end = events.partition_point(|event| execution_event_block_v29(event.site()) <= id);
        let floor = budget.storage();
        let construction = (|| {
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of::<Vec<(usize, Option<SsaValueV1>)>>(),
                argument_product_v1(
                    2,
                    std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
                )?,
                argument_product_v1(10, std::mem::size_of::<usize>())?,
                std::mem::size_of::<Option<Option<SsaValueV1>>>(),
                std::mem::size_of::<bool>(),
            ])?)?;
            emission_vec_v1::<(usize, Option<SsaValueV1>)>(end - first, budget)
        })();
        let storage = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let mut changes = match construction {
            Ok(changes) => changes,
            Err(error) => {
                let _ = budget.release_storage(storage);
                return Err(error);
            }
        };
        let result = (|| {
            for index in first..end {
                budget.charge_work(8)?;
                let event = &self.occurrences.events()[index];
                if event.site() != (ExecutionSiteV29::Terminator { block: id })
                    || !matches!(event.operand(), ExecutionOperandV29::AssertMessage(_))
                    || !event.is_reachable()
                {
                    return Err(execution_availability_error_v29());
                }
                let local = event.event().variable().get() as usize;
                let managed =
                    self.cfg.nominal_locals[local] != 0 || self.cfg.reference_locals[local];
                let changed = if managed {
                    match event.resolved() {
                        Some(SsaResolvedEventV1::Use { value, .. })
                            if self.current[local] == Some(value) =>
                        {
                            None
                        }
                        Some(SsaResolvedEventV1::Kill { previous, .. })
                            if self.current[local] == previous =>
                        {
                            Some(previous)
                        }
                        _ => return Err(execution_availability_error_v29()),
                    }
                } else {
                    None
                };
                // Debit the rollback before changing the failure-only state.
                if changed.is_some() {
                    budget.charge_work(1)?;
                }
                self.claim_events(&[index], budget)?;
                if let Some(previous) = changed {
                    changes.push((local, previous));
                    self.current[local] = None;
                }
            }
            Ok(())
        })();
        for (local, previous) in changes.drain(..).rev() {
            self.current[local] = previous;
        }
        drop(changes);
        let release = budget.release_storage(storage);
        result.and(release)
    }
}

fn execution_site_v29(block: SemanticBlockIdV1, statement: Option<u32>) -> ExecutionSiteV29 {
    let block = SsaBlockIdV1::new(block.index());
    match statement {
        Some(statement) => ExecutionSiteV29::Statement { block, statement },
        None => ExecutionSiteV29::Terminator { block },
    }
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn use_source_place_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.use_source_place_with_role_v29(
            block,
            statement,
            ExecutionOperandV29::RvaluePlace,
            place,
        )
    }

    fn use_source_place_with_role_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        role: ExecutionOperandV29,
        place: &SemanticPlaceV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !self.execution_cfg_local_v29(place.local().index() as usize)
            && !self.execution_local_v29(place.local())?
        {
            return Ok(());
        }
        self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_mut()
                .ok_or_else(execution_availability_error_v29)?;
            let site = execution_site_v29(block, statement);
            budget.charge_work(1)?;
            if !scoped_object_original_place_v29(cursor.function, site, role)
                .is_some_and(|original| std::ptr::eq(original, place))
            {
                return Err(execution_availability_error_v29());
            }
            let definition = cursor.use_place(site, role, place, false, budget)?;
            check_source_use_archive_v29(
                cursor,
                &this.control_flow_ssa.cfg_carriers,
                &this.locals,
                &this.semantic_ssa_bindings,
                site,
                role,
                place,
                definition,
                budget,
            )
        })
    }

    fn execution_local_v29(
        &mut self,
        local: SemanticLocalIdV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        // Only the test adapters omit the shared ledger. This does not bypass
        // the source cursor or archive checks on execution-bearing operands.
        #[cfg(test)]
        if self.emission_work.is_none() {
            return Ok(self
                .locals
                .get(local.index() as usize)
                .and_then(Option::as_ref)
                .is_some_and(semantic_binding_contains_execution_v29));
        }
        self.with_emission_budget_v1(|this, budget| {
            match this
                .locals
                .get(local.index() as usize)
                .and_then(Option::as_ref)
            {
                Some(binding) => execution_binding_contains_paid_v29(binding, budget),
                None => Ok(false),
            }
        })
    }

    fn lower_source_operand_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        role: Option<ExecutionOperandV29>,
        operand: &SemanticOperandV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let role = if role.is_none() && statement.is_none() && self.scoped_memory.is_some() {
            self.with_emission_budget_v1(|this, budget| {
                let Some(source) = this.function.blocks().get(block.index() as usize) else {
                    return Err(scoped_memory_error_v29());
                };
                let SemanticTerminatorKindV1::Call(call) = source.terminator().kind() else {
                    return Ok(None);
                };
                budget.charge_work(call.arguments().len())?;
                call.arguments()
                    .iter()
                    .position(|original| std::ptr::eq(original, operand))
                    .map(|index| {
                        u32::try_from(index)
                            .map(ExecutionOperandV29::CallArgument)
                            .map_err(|_| ArgumentResourceV1::Arithmetic.into())
                    })
                    .transpose()
            })?
        } else {
            role
        };
        let site = execution_site_v29(block, statement);
        let role = role.filter(|&role| {
            scoped_source_operand_v29(self.function, site, role)
                .is_some_and(|source| std::ptr::eq(source, operand))
        });
        self.with_scoped_source_memory_frame_v29(
            ScopedMemoryFrameV29::operand(site, role),
            |this| {
                if let SemanticOperandV1::Move(place) | SemanticOperandV1::Copy(place) = operand
                    && (this.execution_cfg_local_v29(place.local().index() as usize)
                        || this.execution_local_v29(place.local())?)
                {
                    let role = role.ok_or_else(execution_availability_error_v29)?;
                    this.with_emission_budget_v1(|this, budget| {
                        let cursor = this
                            .execution
                            .as_mut()
                            .ok_or_else(execution_availability_error_v29)?;
                        if !cursor
                            .retained_operand(site, role)
                            .is_some_and(|source| std::ptr::eq(source, operand))
                        {
                            return Err(execution_availability_error_v29());
                        }
                        let definition = cursor.use_place(
                            site,
                            role,
                            place,
                            matches!(operand, SemanticOperandV1::Move(_)),
                            budget,
                        )?;
                        check_source_use_archive_v29(
                            cursor,
                            &this.control_flow_ssa.cfg_carriers,
                            &this.locals,
                            &this.semantic_ssa_bindings,
                            site,
                            role,
                            place,
                            definition,
                            budget,
                        )
                    })?;
                }
                this.lower_operand_inner_v1(block, statement, operand, operations)
            },
        )
    }
}

fn execution_binding_contains_paid_v29(
    binding: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    // Pay the full tree, including any later nominal validation/transport walk.
    budget.charge_work(4)?;
    let mut found = false;
    match binding {
        SemanticValueBindingV1::SourceReference(_) | SemanticValueBindingV1::SourceInactive(_) => {
            found = true;
        }
        SemanticValueBindingV1::Aggregate(fields) => {
            for field in fields {
                found |= execution_binding_contains_paid_v29(field, budget)?;
            }
        }
        SemanticValueBindingV1::Enum { payloads, .. } => {
            for fields in payloads.values() {
                budget.charge_work(1)?;
                for field in fields {
                    found |= execution_binding_contains_paid_v29(field, budget)?;
                }
            }
        }
        _ => found = semantic_binding_contains_execution_v29(binding),
    }
    Ok(found)
}

fn execution_archive_object_pointer_same_v29(
    left_id: ValueId,
    left: &Type,
    right_id: ValueId,
    right: &Type,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(6)?;
    let (Type::Pointer(left_pointer), Type::Pointer(right_pointer)) = (left, right) else {
        return Ok(false);
    };
    if left_id != right_id
        || left_pointer.address_space != AddressSpace::Private
        || right_pointer.address_space != AddressSpace::Private
        || !matches!(
            left_pointer.access,
            AccessMode::ReadOnly | AccessMode::ReadWrite
        )
        || !matches!(*left_pointer.pointee, Type::StorageObject(_))
        || !matches!(*right_pointer.pointee, Type::StorageObject(_))
    {
        return Ok(false);
    }
    invocation_equal_types_v1(left, right, budget)
}

fn check_execution_archive_v29(
    locals: &[Option<SemanticValueBindingV1>],
    archive: &SemanticSsaBindingsV1,
    place: &SemanticPlaceV1,
    definition: SsaValueV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    // The map is private to one instance's emitter. SSA IDs never join two
    // helper instances, even when they share a source declaration and plan.
    budget.charge_work(archive.len().checked_ilog2().unwrap_or(0).saturating_add(2) as usize)?;
    let mut held = locals
        .get(place.local().index() as usize)
        .and_then(Option::as_ref)
        .ok_or_else(execution_availability_error_v29)?;
    let mut original = archive
        .get(&definition)
        .ok_or_else(execution_availability_error_v29)?;
    budget.charge_work(place.projections().len())?;
    for projection in place.projections() {
        match (projection.kind(), held, original) {
            (
                SemanticProjectionKindV1::Dereference,
                SemanticValueBindingV1::Value {
                    id: left_id,
                    ty: left,
                },
                SemanticValueBindingV1::Value {
                    id: right_id,
                    ty: right,
                },
            ) => {
                if !execution_archive_object_pointer_same_v29(
                    *left_id, left, *right_id, right, budget,
                )? {
                    return Err(execution_availability_error_v29());
                }
                // Only the archived holder is established here. Original
                // source access/schema/currentness checks still own the suffix.
                return Ok(());
            }
            (
                SemanticProjectionKindV1::Dereference,
                SemanticValueBindingV1::SourceReference(left),
                SemanticValueBindingV1::SourceReference(right),
            ) if left == right => {
                budget.charge_work(argument_product_v1(left.values.len(), 4)?)?;
                // The exact holder is archived. The checked place resolver
                // validates the pointee and every remaining projection.
                return Ok(());
            }
            (
                SemanticProjectionKindV1::Field(index),
                SemanticValueBindingV1::Aggregate(left),
                SemanticValueBindingV1::Aggregate(right),
            ) => {
                held = left
                    .get(index as usize)
                    .ok_or_else(execution_availability_error_v29)?;
                original = right
                    .get(index as usize)
                    .ok_or_else(execution_availability_error_v29)?;
            }
            (
                SemanticProjectionKindV1::Dereference,
                SemanticValueBindingV1::ExecutionBorrow(left),
                SemanticValueBindingV1::ExecutionBorrow(right),
            ) if left == right => {}
            _ => return Err(execution_availability_error_v29()),
        }
    }
    fn same(
        left: &SemanticValueBindingV1,
        right: &SemanticValueBindingV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        Ok(match (left, right) {
            (
                SemanticValueBindingV1::Value {
                    id: left_id,
                    ty: left,
                },
                SemanticValueBindingV1::Value {
                    id: right_id,
                    ty: right,
                },
            ) if matches!(left, Type::Pointer(pointer) if matches!(*pointer.pointee, Type::StorageObject(_)))
                || matches!(right, Type::Pointer(pointer) if matches!(*pointer.pointee, Type::StorageObject(_))) =>
            {
                execution_archive_object_pointer_same_v29(*left_id, left, *right_id, right, budget)?
            }
            (
                SemanticValueBindingV1::Value {
                    ty: Type::Pointer(pointer),
                    ..
                },
                _,
            )
            | (
                _,
                SemanticValueBindingV1::Value {
                    ty: Type::Pointer(pointer),
                    ..
                },
            ) if matches!(*pointer.pointee, Type::StorageObject(_)) => false,
            (
                SemanticValueBindingV1::SourceInactive(left),
                SemanticValueBindingV1::SourceInactive(right),
            ) => {
                budget.charge_work(argument_sum_v1(&[
                    argument_product_v1(left.values.len(), 4)?,
                    8,
                ])?)?;
                left.owner == right.owner
                    && left.source == right.source
                    && left.ssa == right.ssa
                    && left.root == right.root
                    && left.node == right.node
                    && left.source_type == right.source_type
                    && source_reference_inactive_values_same_v29(
                        &left.values,
                        &right.values,
                        budget,
                    )?
            }
            (
                SemanticValueBindingV1::SourceReference(left),
                SemanticValueBindingV1::SourceReference(right),
            ) => {
                budget.charge_work(argument_product_v1(left.values.len(), 4)?)?;
                left == right
            }
            (SemanticValueBindingV1::Execution(left), SemanticValueBindingV1::Execution(right)) => {
                left == right
            }
            (
                SemanticValueBindingV1::ExecutionBorrow(left),
                SemanticValueBindingV1::ExecutionBorrow(right),
            ) => left == right,
            (SemanticValueBindingV1::Aggregate(left), SemanticValueBindingV1::Aggregate(right))
                if left.len() == right.len() =>
            {
                for (left, right) in left.iter().zip(right) {
                    if !same(left, right, budget)? {
                        return Ok(false);
                    }
                }
                true
            }
            (
                SemanticValueBindingV1::MovedExecution | SemanticValueBindingV1::Unmaterialized,
                _,
            ) => false,
            _ => {
                !execution_binding_contains_paid_v29(left, budget)?
                    && !execution_binding_contains_paid_v29(right, budget)?
            }
        })
    }
    if same(held, original, budget)? {
        Ok(())
    } else {
        Err(execution_availability_error_v29())
    }
}
