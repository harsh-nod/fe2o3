use fe2o3_pliron::{
    ProductionSemanticSsaEventRoleV1 as ExecutionEventV29,
    ProductionSemanticSsaFunctionOccurrencesV1,
    ProductionSemanticSsaOccurrenceSiteV1 as ExecutionSiteV29,
    ProductionSemanticSsaOperandRoleV1 as ExecutionOperandV29,
};
use production_call_instances_v1::ProductionCallInstancePlanV1 as ExecutionInstancesV29;

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
    index: Vec<UnitLocalSourceIndexV1>,
    claimed: Vec<bool>,
    current: Vec<Option<SsaValueV1>>,
    seen: Vec<bool>,
    visited: Vec<bool>,
    block: Option<SsaBlockIdV1>,
    cfg: ExecutionCfgV29<'a>,
    events: ExecutionEventsV29,
    parameters: Option<PreparedExecutionParametersV29<'a>>,
    #[cfg(test)]
    entry_seeds: Vec<(u32, SemanticValueBindingV1)>,
    #[cfg(test)]
    skipped_event: Option<usize>,
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "V29 production admission remains gated on source custody"
    )
)]
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

impl<'a> ExecutionAvailabilityV29<'a> {
    fn new(
        instances: &ExecutionInstancesV29<'a>,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let row = instances
            .instance(instance)
            .ok_or_else(execution_availability_error_v29)?;
        let occurrences = instances
            .occurrences(instance)
            .ok_or_else(execution_availability_error_v29)?;
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
        let cfg = ExecutionCfgV29::new(
            instances.owner().source_semantic().types(),
            row.declaration(),
            row.ssa(),
            &occurrences,
            budget,
        )?;
        let events =
            ExecutionEventsV29::new(&occurrences, row.declaration(), &cfg.nominal_locals, budget)?;
        Ok(Self {
            ledger: budget.work_ledger_identity_v1(),
            instance,
            source: ExecutionCallSourceV29::from_instances(instances, budget)?,
            function_id: row.function(),
            function: row.declaration(),
            ssa: row.ssa(),
            occurrences,
            index,
            claimed,
            current,
            seen,
            visited,
            block: None,
            cfg,
            events,
            parameters: None,
            #[cfg(test)]
            entry_seeds: Vec::new(),
            #[cfg(test)]
            skipped_event: None,
        })
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
            || !self.ssa.plan().is_reachable(block)
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
                    if self.claimed[index]
                        || !event.is_reachable()
                        || !event.is_promoted()
                        || event.resolved().is_none()
                    {
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
        match (site, role) {
            (ExecutionSiteV29::Terminator { block }, ExecutionOperandV29::CallArgument(index)) => {
                let SemanticTerminatorKindV1::Call(call) = self
                    .function
                    .blocks()
                    .get(block.get() as usize)?
                    .terminator()
                    .kind()
                else {
                    return None;
                };
                call.arguments().get(index as usize)
            }
            (ExecutionSiteV29::Terminator { block }, role) => match self
                .function
                .blocks()
                .get(block.get() as usize)?
                .terminator()
                .kind()
            {
                SemanticTerminatorKindV1::SwitchInt { discriminant, .. }
                    if role == ExecutionOperandV29::SwitchDiscriminant =>
                {
                    Some(discriminant)
                }
                SemanticTerminatorKindV1::Assert {
                    condition, message, ..
                } => match role {
                    ExecutionOperandV29::AssertCondition => Some(condition),
                    ExecutionOperandV29::AssertMessage(index) => {
                        execution_assert_operand_v29(message, index)
                    }
                    _ => None,
                },
                _ => None,
            },
            (
                ExecutionSiteV29::Statement { block, statement },
                ExecutionOperandV29::RvalueOperand(index),
            ) => {
                let SemanticStatementKindV1::Assign(assignment) = self
                    .function
                    .blocks()
                    .get(block.get() as usize)?
                    .statements()
                    .get(statement as usize)?
                    .kind()
                else {
                    return None;
                };
                match assignment.value().kind() {
                    SemanticRvalueKindV1::Use(operand)
                    | SemanticRvalueKindV1::Unary { operand, .. }
                    | SemanticRvalueKindV1::Cast { operand, .. }
                        if index == 0 =>
                    {
                        Some(operand)
                    }
                    SemanticRvalueKindV1::Binary { left, right, .. } => match index {
                        0 => Some(left),
                        1 => Some(right),
                        _ => None,
                    },
                    SemanticRvalueKindV1::CheckedBinary(operation) => match index {
                        0 => Some(operation.left()),
                        1 => Some(operation.right()),
                        _ => None,
                    },
                    SemanticRvalueKindV1::Aggregate(aggregate) => {
                        aggregate.operands().get(index as usize)
                    }
                    _ => None,
                }
            }
            (ExecutionSiteV29::Statement { block, statement }, ExecutionOperandV29::Assume) => {
                let SemanticStatementKindV1::Assume(condition) = self
                    .function
                    .blocks()
                    .get(block.get() as usize)?
                    .statements()
                    .get(statement as usize)?
                    .kind()
                else {
                    return None;
                };
                Some(condition)
            }
            _ => None,
        }
    }
}

fn execution_site_v29(block: SemanticBlockIdV1, statement: Option<u32>) -> ExecutionSiteV29 {
    let block = SsaBlockIdV1::new(block.index());
    match statement {
        Some(statement) => ExecutionSiteV29::Statement { block, statement },
        None => ExecutionSiteV29::Terminator { block },
    }
}

impl SemanticFunctionLoweringV1<'_> {
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
        if let SemanticOperandV1::Move(place) | SemanticOperandV1::Copy(place) = operand
            && (self.execution_cfg_local_v29(place.local().index() as usize)
                || self.execution_local_v29(place.local())?)
        {
            let role = role.ok_or_else(execution_availability_error_v29)?;
            self.with_emission_budget_v1(|this, budget| {
                let cursor = this
                    .execution
                    .as_mut()
                    .ok_or_else(execution_availability_error_v29)?;
                let site = execution_site_v29(block, statement);
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
                check_execution_archive_v29(
                    &this.locals,
                    &this.semantic_ssa_bindings,
                    place,
                    definition,
                    budget,
                )
            })?;
        }
        self.lower_operand_inner_v1(block, statement, operand, operations)
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

fn check_execution_archive_v29(
    locals: &[Option<SemanticValueBindingV1>],
    archive: &BTreeMap<SsaValueV1, SemanticValueBindingV1>,
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
