// Box each actual binding once instead of reserving its full payload in every
// possible BTree split slot. Borrowers still see the complete original binding.
#[derive(Default)]
#[cfg_attr(test, derive(Clone, Debug))]
struct SemanticSsaBindingsV1 {
    owned: BTreeMap<SsaValueV1, Box<SemanticValueBindingV1>>,
}

impl SemanticSsaBindingsV1 {
    fn len(&self) -> usize {
        self.owned.len()
    }

    fn is_empty(&self) -> bool {
        self.owned.is_empty()
    }

    fn contains_key(&self, value: &SsaValueV1) -> bool {
        self.owned.contains_key(value)
    }

    fn get(&self, value: &SsaValueV1) -> Option<&SemanticValueBindingV1> {
        self.owned.get(value).map(Box::as_ref)
    }
}

#[cfg(test)]
impl SemanticSsaBindingsV1 {
    fn iter(&self) -> impl Iterator<Item = (&SsaValueV1, &SemanticValueBindingV1)> {
        self.owned.iter().map(|(value, binding)| (value, binding.as_ref()))
    }

    fn values(&self) -> impl Iterator<Item = &SemanticValueBindingV1> {
        self.owned.values().map(Box::as_ref)
    }

    fn insert(
        &mut self,
        value: SsaValueV1,
        binding: SemanticValueBindingV1,
    ) -> Option<SemanticValueBindingV1> {
        self.owned.insert(value, Box::new(binding)).map(|binding| *binding)
    }

    fn remove(&mut self, value: &SsaValueV1) -> Option<SemanticValueBindingV1> {
        self.owned.remove(value).map(|binding| *binding)
    }

    fn clear(&mut self) {
        self.owned.clear();
    }
}

#[cfg(test)]
impl<const N: usize> From<[(SsaValueV1, SemanticValueBindingV1); N]> for SemanticSsaBindingsV1 {
    fn from(bindings: [(SsaValueV1, SemanticValueBindingV1); N]) -> Self {
        Self {
            owned: bindings.into_iter().map(|(value, binding)| (value, Box::new(binding))).collect(),
        }
    }
}

#[cfg(test)]
impl std::ops::Index<&SsaValueV1> for SemanticSsaBindingsV1 {
    type Output = SemanticValueBindingV1;

    fn index(&self, value: &SsaValueV1) -> &Self::Output {
        self.owned[value].as_ref()
    }
}

// This owns emitted locators, not source-value or pointer-provenance proofs.
// The original map moves once from the emitter through expanded-root checking.
struct ExecutionArchiveV29 {
    subject: ScopedInitializationSubjectV29,
    plan: fe2o3_mir_model::SsaPlanIdentityV1,
    credit: ExecutionArchiveCreditV29,
    bindings: SemanticSsaBindingsV1,
    #[cfg(test)]
    locals: Vec<Option<SemanticValueBindingV1>>,
    #[cfg(test)]
    retained_seeds: Vec<Option<SemanticExecutionBindingV29>>,
}

// The owner, not a field moved out of it, reaches the paid destruction boundary.
// Dropping never refunds; only the original root scope may settle its credit.
impl Drop for ExecutionArchiveV29 {
    fn drop(&mut self) {}
}

#[cfg(test)]
type RootExecutionArchiveObserverV29 = fn(
    &mut PendingScopedRootEmissionV29,
    &ExecutionInstancesV29<'_>,
    &SourceReferencePlanV29<'_, '_>,
    &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1>;

#[cfg(test)]
thread_local! {
    static ROOT_EXECUTION_ARCHIVE_OBSERVER_V29: std::cell::Cell<Option<RootExecutionArchiveObserverV29>> = const { std::cell::Cell::new(None) };
}

#[derive(Clone, Copy)]
struct ExecutionArchiveCreditV29 {
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    bytes: usize,
}

impl ExecutionArchiveCreditV29 {
    fn new(budget: &dyn SemanticEmissionBudgetV1) -> Result<Self, ProductionSemanticKirErrorV1> {
        Ok(Self {
            slot: budget
                .prepared_input_slot_v1()
                .ok_or(ArgumentResourceV1::Accounting)?,
            ledger: budget.work_ledger_identity_v1(),
            bytes: 0,
        })
    }

    fn check(
        &self,
        budget: &dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if budget.prepared_input_slot_v1() != Some(self.slot)
            || budget.work_ledger_identity_v1() != self.ledger
            || budget.storage() < self.bytes
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum ExecutionArchiveDefinitionSiteV29 {
    Invocation { local: u32 },
    BlockArgument { block: SsaBlockIdV1, local: u32 },
    Definition { site: ExecutionSiteV29, local: u32 },
    Edge { edge: SsaEdgeIdV1, local: u32 },
}

fn execution_archive_error_v29() -> ProductionSemanticKirErrorV1 {
    unsupported(
        0,
        None,
        None,
        "execution archive differs from its original source definition",
    )
}

fn execution_archive_argument_v29(
    arguments: &[SsaArgumentV1],
    local: u32,
    value: SsaValueV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    // The admitted SSA plan stores arguments in original variable order.
    charge_execution_cfg_lookup_v29(arguments.len(), budget)?;
    if arguments
        .binary_search_by_key(&local, |argument| argument.variable().get())
        .ok()
        .and_then(|index| arguments.get(index))
        .is_none_or(|argument| argument.value() != value)
    {
        return Err(execution_archive_error_v29());
    }
    Ok(())
}

impl ExecutionAvailabilityV29<'_> {
    fn check_archive_definition_v29(
        &self,
        value: SsaValueV1,
        site: ExecutionArchiveDefinitionSiteV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_ledger(budget)?;
        if let Some(references) = self.references {
            budget.source_reference_owner_v29(references.plan)?;
        }
        budget.charge_work(4)?;
        let block = match site {
            ExecutionArchiveDefinitionSiteV29::Invocation { .. } => self.function.entry(),
            ExecutionArchiveDefinitionSiteV29::BlockArgument { block, .. } => {
                SemanticBlockIdV1::from_index(block.get())
            }
            ExecutionArchiveDefinitionSiteV29::Definition { site, .. } => {
                SemanticBlockIdV1::from_index(execution_event_block_v29(site).get())
            }
            ExecutionArchiveDefinitionSiteV29::Edge { edge, .. } => {
                SemanticBlockIdV1::from_index(edge.source().get())
            }
        };
        if !self.source_block_reachable_v29(block, budget)? {
            return Err(execution_archive_error_v29());
        }
        match site {
            ExecutionArchiveDefinitionSiteV29::Invocation { local } => {
                execution_archive_argument_v29(
                    self.ssa.plan().entry_definitions(),
                    local,
                    value,
                    budget,
                )
            }
            ExecutionArchiveDefinitionSiteV29::BlockArgument { block, local } => {
                let variables = self
                    .ssa
                    .plan()
                    .transport_variables(block)
                    .ok_or_else(execution_archive_error_v29)?;
                charge_execution_cfg_lookup_v29(variables.len(), budget)?;
                if self.block != Some(block)
                    || value
                        != (SsaValueV1::BlockArgument {
                            block,
                            variable: fe2o3_mir_model::SsaVariableIdV1::new(local),
                        })
                    || variables
                        .binary_search_by_key(&local, |variable| variable.get())
                        .is_err()
                {
                    return Err(execution_archive_error_v29());
                }
                Ok(())
            }
            ExecutionArchiveDefinitionSiteV29::Edge { edge, local } => {
                if self.block != Some(edge.source()) {
                    return Err(execution_archive_error_v29());
                }
                let successors = self.occurrences.successors();
                charge_execution_cfg_lookup_v29(successors.len(), budget)?;
                let ordinal = successors.binary_search_by_key(&edge, |row| row.id())
                    .map_err(|_| execution_archive_error_v29())?;
                if !self.control.successor_reachable(
                    SemanticBlockIdV1::from_index(edge.source().get()),
                    successors[ordinal].edge().role(),
                    budget,
                )? {
                    return Err(execution_archive_error_v29());
                }
                execution_archive_argument_v29(
                    self.ssa
                        .plan()
                        .edge_definitions(edge)
                        .ok_or_else(execution_archive_error_v29)?,
                    local,
                    value,
                    budget,
                )
            }
            ExecutionArchiveDefinitionSiteV29::Definition { site, local } => {
                if self.block != Some(execution_event_block_v29(site))
                    || self.current.get(local as usize) != Some(&Some(value))
                {
                    return Err(execution_archive_error_v29());
                }
                let mut selected = None;
                for operand in [
                    ExecutionOperandV29::Destination,
                    ExecutionOperandV29::ElidedBorrowDestination,
                ] {
                    let key = unit_local_source_key_v1(
                        site,
                        operand,
                        Some(ExecutionEventV29::DestinationDefine),
                    );
                    charge_execution_cfg_lookup_v29(self.index.len(), budget)?;
                    if let Ok(index) = self.index.binary_search_by_key(&key, |entry| entry.key) {
                        if selected.replace(self.index[index].index).is_some() {
                            return Err(execution_archive_error_v29());
                        }
                    }
                }
                let index = selected.ok_or_else(execution_archive_error_v29)?;
                let event = self
                    .occurrences
                    .events()
                    .get(index)
                    .ok_or_else(execution_archive_error_v29)?;
                if !event.is_reachable()
                    || !event.is_promoted()
                    || self.claimed.get(index) != Some(&true)
                    || event.resolved()
                        != Some(SsaResolvedEventV1::Define {
                            variable: fe2o3_mir_model::SsaVariableIdV1::new(local),
                            value,
                        })
                {
                    return Err(execution_archive_error_v29());
                }
                Ok(())
            }
        }
    }
}

fn archive_scoped_binding_v29(
    cursor: &ExecutionAvailabilityV29<'_>,
    bindings: &mut SemanticSsaBindingsV1,
    credit: &mut Option<ExecutionArchiveCreditV29>,
    value: SsaValueV1,
    binding: &SemanticValueBindingV1,
    site: ExecutionArchiveDefinitionSiteV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    cursor.check_archive_definition_v29(value, site, budget)?;
    archive_owned_binding_v29(bindings, credit, value, binding, budget)
}

// Internal storage operation only. Its result is not an admitted source archive;
// the sole production caller above first joins the consumed original definition.
fn archive_owned_binding_v29(
    bindings: &mut SemanticSsaBindingsV1,
    credit: &mut Option<ExecutionArchiveCreditV29>,
    value: SsaValueV1,
    binding: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut owned = match *credit {
        Some(owned) => {
            owned.check(budget)?;
            owned
        }
        None => ExecutionArchiveCreditV29::new(budget)?,
    };
    charge_execution_cfg_lookup_v29(bindings.len(), budget)?;
    if bindings.contains_key(&value) {
        return Err(execution_archive_error_v29());
    }
    let map_bytes = execution_cfg_archive_entry_storage_v29(bindings.len())?;
    let next_bytes = argument_sum_v1(&[owned.bytes, map_bytes])?;
    reserve_execution_cfg_archive_v29(bindings.len(), budget)?;
    owned.bytes = next_bytes;
    *credit = Some(owned);
    // This closed clone has no user callback or C2 mutation. Its failed partial
    // backing is dropped by emission_clone_binding_v1 before its own refund.
    let before = budget.storage();
    let copy = emission_clone_binding_v1(binding, budget)?;
    owned.check(budget)?;
    let payload = budget
        .storage()
        .checked_sub(before)
        .ok_or(ArgumentResourceV1::Accounting)?;
    owned.bytes = argument_sum_v1(&[owned.bytes, payload])?;
    *credit = Some(owned);
    insert_semantic_ssa_binding_v1(bindings, 0, None, None, value, copy)
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn take_execution_archive_v29(
        &mut self,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<ExecutionArchiveV29>, ProductionSemanticKirErrorV1> {
        let Some(cursor) = self.execution.as_ref() else {
            if self.semantic_ssa_archive_credit.is_some() {
                return Err(execution_archive_error_v29());
            }
            return Ok(None);
        };
        cursor.check_ledger(budget)?;
        if !cursor.events.finished {
            return Err(execution_archive_error_v29());
        }
        if let Some(references) = cursor.references {
            budget.source_reference_owner_v29(references.plan)?;
        }
        let mut credit = match self.semantic_ssa_archive_credit {
            Some(credit) => {
                credit.check(budget)?;
                credit
            }
            None if self.semantic_ssa_bindings.is_empty() => {
                ExecutionArchiveCreditV29::new(budget)?
            }
            None => return Err(execution_archive_error_v29()),
        };
        let header = std::mem::size_of::<ExecutionArchiveV29>();
        let bytes = argument_sum_v1(&[credit.bytes, header])?;
        budget.charge_work(8)?;
        budget.reserve_storage(header)?;
        credit.bytes = bytes;
        self.semantic_ssa_archive_credit = Some(credit);
        #[cfg(test)]
        let retained_seeds = copy_execution_observation_seeds_v1(&cursor.retained_seeds, budget)?;
        let output = ExecutionArchiveV29 {
            subject: ScopedInitializationSubjectV29::from_cursor(cursor),
            plan: cursor.ssa.plan().identity(),
            credit,
            bindings: std::mem::take(&mut self.semantic_ssa_bindings),
            #[cfg(test)]
            locals: std::mem::take(&mut self.locals),
            #[cfg(test)]
            retained_seeds,
        };
        self.semantic_ssa_archive_credit = None;
        Ok(Some(output))
    }
}

impl ExecutionArchiveV29 {
    fn lookup_original_v29<'archive>(
        &'archive self,
        instances: &ExecutionInstancesV29<'_>,
        instance: ProductionCallInstanceIdV1,
        value: SsaValueV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'archive SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.check_original_v29(instances, instance, budget)?;
        charge_execution_cfg_lookup_v29(self.bindings.len(), budget)?;
        self.bindings
            .get(&value)
            .ok_or_else(execution_archive_error_v29)
    }

    fn check_original_v29(
        &self,
        instances: &ExecutionInstancesV29<'_>,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.credit.check(budget)?;
        budget.charge_work(7)?;
        let original = instances
            .instance(instance)
            .ok_or_else(execution_archive_error_v29)?;
        if self.subject.instance != instance
            || instances.instance_reachable(instance) != Some(true)
            || self.subject.function != original.function()
            || self.subject.ledger != budget.work_ledger_identity_v1()
            || self.subject.source != ExecutionCallSourceV29::from_instances(instances, budget)?
            || self.plan != original.ssa().plan().identity()
        {
            return Err(execution_archive_error_v29());
        }
        Ok(())
    }
}

// This is archive custody only. The caller must first finish the existing
// source/reference postflight, and any future raw physical census, on this root.
fn check_root_execution_archives_v29(
    pending: &PendingScopedRootEmissionV29,
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    budget.charge_work(argument_sum_v1(&[
        argument_product_v1(pending.sidecars.rows.len(), 2)?,
        4,
    ])?)?;
    if !std::ptr::eq(plan.instances, instances) {
        return Err(execution_archive_error_v29());
    }
    let index = &pending.active_instances;
    index.check_source_plan(instances, &pending.sidecars.rows, budget)?;
    let mut bytes = 0;
    for (ordinal, selected) in index.rows.iter().enumerate() {
        let instance = instances
            .id_at(ordinal)
            .ok_or_else(execution_archive_error_v29)?;
        let Some(selected) = *selected else { continue };
        let sidecar = pending.sidecars.rows.get(selected).ok_or_else(execution_archive_error_v29)?;
        bytes = argument_sum_v1(&[
            bytes,
            check_execution_archive_instance_v29(
                instances,
                instance,
                sidecar.source_call_instance,
                sidecar.execution_observation.as_ref(),
                budget,
            )?,
        ])?;
    }
    if !budget.permits_prepared_input_refund_v1(
        Some(plan),
        plan.slot,
        plan.ledger,
        budget.storage(),
        bytes,
    ) {
        if let Some(root) = plan.storage_root.as_ref() {
            root.deny_active_root_refund();
        }
        return Err(ArgumentResourceV1::Accounting.into());
    }
    Ok(bytes)
}

fn check_execution_archive_instance_v29(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    emitted_instance: Option<ProductionCallInstanceIdV1>,
    archive: Option<&ExecutionArchiveV29>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    if emitted_instance != Some(instance) {
        return Err(execution_archive_error_v29());
    }
    match (instances.instance_reachable(instance), archive) {
        (Some(true), Some(archive)) => {
            archive.check_original_v29(instances, instance, budget)?;
            // Prepay destruction before the first backing is dropped.
            budget.charge_work(archive.bindings.len())?;
            Ok(archive.credit.bytes)
        }
        (Some(false), None) => Ok(0),
        _ => Err(execution_archive_error_v29()),
    }
}

fn discard_root_execution_archives_v29(
    pending: &mut PendingScopedRootEmissionV29,
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let bytes = check_root_execution_archives_v29(pending, instances, plan, budget)?;
    let required = budget.storage();
    for row in &mut pending.sidecars.rows {
        drop(row.execution_observation.take());
    }
    budget.release_emission_service_storage_v1(Some(plan), plan.slot, plan.ledger, required, bytes)
}
