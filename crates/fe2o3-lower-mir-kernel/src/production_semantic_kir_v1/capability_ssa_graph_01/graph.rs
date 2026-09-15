use super::*;
use fe2o3_mir_model::{SsaConstructionPlanV1, SsaEdgeIdV1};

include!("region_acyclic.rs");
include!("loan_region_reuse.rs");
include!("owner_invalidation_index.rs");
include!("path_region_scratch.rs");
include!("definition_memo.rs");

mod enum_guard {
    include!("enum_guard.rs");
}

mod endpoint_scc {
    include!("endpoint_scc.rs");
}

mod query_reuse {
    include!("query_reuse.rs");
}
use query_reuse::{CapabilityQueryReuseV1, insertion_work, lookup_work};

mod absent_use_observation {
    include!("absent_use_observation.rs");
}

mod work_failure_observation {
    include!("work_failure_observation.rs");
}

mod work_profile {
    include!("work_profile.rs");
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct CapabilityDefinitionSiteV1 {
    pub(super) block: u32,
    pub(super) statement: Option<u32>,
    pub(super) local: u32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct CapabilityLoanV1 {
    pub(super) borrow: CapabilityDefinitionSiteV1,
    pub(super) owner_local: u32,
    pub(super) owner_value: SsaValueV1,
}

/// SSA identities and conservative, ordered storage checks. Multiple versions
/// of a local in one block reject until the shared event-origin API is supplied.
pub(super) struct CapabilitySsaGraphV1<'a> {
    pub(super) body: &'a SemanticFunctionDeclV1,
    pub(super) ssa: &'a SsaConstructionPlanV1,
    remaining: usize,
    limit: usize,
    reuse: CapabilityQueryReuseV1<'a>,
    work_profile: Option<Box<work_profile::Profile>>,
}

impl<'a> CapabilitySsaGraphV1<'a> {
    pub(super) fn new(
        body: &'a SemanticFunctionDeclV1,
        ssa: &'a SsaConstructionPlanV1,
        max_work: usize,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let mut graph = Self {
            body,
            ssa,
            remaining: max_work,
            limit: max_work,
            reuse: CapabilityQueryReuseV1::default(),
            work_profile: work_profile::Profile::configured(),
        };
        graph.charge(query_reuse::header_words())?;
        graph.charge(body.blocks().len())?;
        graph.charge(body.locals().len())?;
        Ok(graph)
    }

    pub(super) fn selected_variant(
        &mut self,
        types: &'a [SemanticTypeDeclV1],
        value: SsaValueV1,
        enum_type: SemanticTypeIdV1,
        variant: u32,
        use_block: u32,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        self.charge(5)?;
        let unbound = match self.reuse.enum_guards.owner {
            None => true,
            Some((body, ssa, original_types)) => {
                if !std::ptr::eq(body, self.body)
                    || !std::ptr::eq(ssa, self.ssa)
                    || !std::ptr::eq(original_types.as_ptr(), types.as_ptr())
                    || original_types.len() != types.len()
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                false
            }
        };
        let key = (value, enum_type, variant, use_block);
        self.charge(lookup_work(self.reuse.enum_guards.rows.len()))?;
        if let Some(result) = self.reuse.enum_guards.rows.get(&key) {
            return Ok(*result);
        }
        let result = enum_guard::selected_variant(self, types, value, enum_type, variant, use_block)?;
        self.charge(insertion_work::<enum_guard::Key, bool>(self.reuse.enum_guards.rows.len()))?;
        if unbound {
            self.charge(4)?;
        }
        // Publish the completed answer and first owner only after every debit.
        self.reuse.enum_guards.rows.insert(key, result);
        if unbound {
            self.reuse.enum_guards.owner = Some((self.body, self.ssa, types));
        }
        Ok(result)
    }

    #[track_caller]
    pub(super) fn charge(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        let Some(remaining) = self.remaining.checked_sub(amount) else {
            let identity = self.body.identity();
            let failure = work_failure_observation::Failure {
                body: identity.as_bytes(),
                remaining: self.remaining,
                requested: amount,
                limit: self.limit,
                blocks: self.body.blocks().len(),
                locals: self.body.locals().len(),
                cache_rows: [
                    self.reuse.uses.len(),
                    self.reuse.definitions.len(),
                    self.reuse.reachability.len(),
                    self.reuse.loans.len(),
                ],
            };
            let caller = std::panic::Location::caller();
            work_failure_observation::emit(failure, caller);
            if let Some(profile) = &self.work_profile {
                profile.emit(failure, caller);
            }
            return Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                actual: self.limit.saturating_add(1),
                limit: self.limit,
            });
        };
        self.remaining = remaining;
        if let Some(profile) = &mut self.work_profile {
            profile.record(amount, std::panic::Location::caller());
        }
        Ok(())
    }

    #[track_caller]
    pub(super) fn use_value(
        &mut self,
        block: u32,
        local: u32,
    ) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
        let key = (block, local);
        self.charge(lookup_work(self.reuse.uses.len()))?;
        if let Some(result) = self.reuse.uses.get(&key).copied() {
            return Ok(result);
        }
        let result = self.use_value_uncached(block, local)?;
        self.charge(insertion_work::<(u32, u32), SsaValueV1>(self.reuse.uses.len()))?;
        self.reuse.uses.insert(key, result);
        Ok(result)
    }

    #[track_caller]
    fn use_value_uncached(
        &mut self,
        block: u32,
        local: u32,
    ) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
        let events = self
            .ssa
            .resolved_events(SsaBlockIdV1::new(block))
            .ok_or_else(|| reject("capability use has no reachable SSA events"))?;
        self.charge(events.len())?;
        let mut result = None;
        for (_, event) in events {
            if let SsaResolvedEventV1::Use { variable, value } = event
                && variable.get() == local
            {
                if result.replace(*value).is_some_and(|old| old != *value) {
                    return Err(reject(
                        "capability reference has multiple SSA versions in one block",
                    ));
                }
            }
        }
        let Some(value) = result else {
            absent_use_observation::emit(
                self.body.identity().as_bytes(),
                block,
                local,
                events.len(),
                self.remaining,
                std::panic::Location::caller(),
            );
            return Err(reject("capability reference has no exact SSA use"));
        };
        Ok(value)
    }

    fn definition_uncached(
        &mut self,
        value: SsaValueV1,
    ) -> Result<CapabilityDefinitionSiteV1, ProductionSemanticKirErrorV1> {
        if let Some(source) = self.reuse.definition_source {
            return self.definition_from_index(&source, value);
        }
        self.definition_scan(value)
    }

    fn definition_scan(
        &mut self,
        value: SsaValueV1,
    ) -> Result<CapabilityDefinitionSiteV1, ProductionSemanticKirErrorV1> {
        let mut result = None;
        for block in self.ssa.reverse_postorder() {
            let body = &self.body.blocks()[block.get() as usize];
            let events = self
                .ssa
                .resolved_events(*block)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            self.charge(events.len())?;
            for (_, event) in events {
                let SsaResolvedEventV1::Define {
                    variable,
                    value: candidate,
                } = event
                else {
                    continue;
                };
                if *candidate != value {
                    continue;
                }
                self.charge(events.len())?;
                self.charge(body.statements().len())?;
                if events.iter().filter(|(_, event)| matches!(event, SsaResolvedEventV1::Define { variable: local, .. } if local == variable)).count() != 1 {
                    return Err(reject("capability definition has multiple source assignments in one block"));
                }
                let mut site = None;
                for (index, statement) in body.statements().iter().enumerate() {
                    if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                        && assignment.destination().local().index() == variable.get()
                    {
                        if !assignment.destination().projections().is_empty()
                            || site.replace(index as u32).is_some()
                        {
                            return Err(reject(
                                "capability definition is projected or overwritten",
                            ));
                        }
                    }
                }
                let statement = site.ok_or_else(|| {
                    reject("capability definition has no checked source assignment")
                })?;
                if result
                    .replace(CapabilityDefinitionSiteV1 {
                        block: block.get(),
                        statement: Some(statement),
                        local: variable.get(),
                    })
                    .is_some()
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
            }
            for ordinal in 0..body.terminator().kind().edge_count() {
                let definitions = self
                    .ssa
                    .edge_definitions(SsaEdgeIdV1::new(*block, ordinal as u32))
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                self.charge(definitions.len())?;
                for definition in definitions {
                    if definition.value() != value {
                        continue;
                    }
                    let SemanticTerminatorKindV1::Call(call) = body.terminator().kind() else {
                        return Err(reject("capability issuer is not a source call"));
                    };
                    if ordinal != 0
                        || !call.destination().is_some_and(|destination| {
                            destination.place().local().index() == definition.variable().get()
                                && destination.place().projections().is_empty()
                        })
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    if result
                        .replace(CapabilityDefinitionSiteV1 {
                            block: block.get(),
                            statement: None,
                            local: definition.variable().get(),
                        })
                        .is_some()
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                }
            }
        }
        result.ok_or_else(|| {
            reject("capability authority originates from an entry parameter or missing definition")
        })
    }

    pub(super) fn incoming(
        &mut self,
        target: u32,
        local: u32,
    ) -> Result<Vec<SsaValueV1>, ProductionSemanticKirErrorV1> {
        let mut values = Vec::new();
        for block in self.ssa.reverse_postorder() {
            let body = &self.body.blocks()[block.get() as usize];
            let mut ordinal = 0;
            body.terminator().kind().try_for_each_edge(|edge| {
                self.charge(1)?;
                if edge.target().index() == target {
                    let arguments = self
                        .ssa
                        .edge_arguments(SsaEdgeIdV1::new(*block, ordinal))
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    self.charge(arguments.len())?;
                    let mut found = None;
                    for argument in arguments {
                        if argument.variable().get() == local
                            && found.replace(argument.value()).is_some()
                        {
                            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                        }
                    }
                    values.push(found.ok_or_else(|| {
                        reject("capability SSA merge lacks its exact edge value")
                    })?);
                }
                ordinal += 1;
                Ok(())
            })?;
        }
        if values.is_empty() {
            return Err(reject("capability SSA merge has no incoming owner"));
        }
        Ok(values)
    }

    pub(super) fn reaches(
        &mut self,
        from: u32,
        to: u32,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let key = (from, to);
        self.charge(lookup_work(self.reuse.reachability.len()))?;
        if let Some(result) = self.reuse.reachability.get(&key).copied() {
            return Ok(result);
        }
        let result = self.reaches_uncached(from, to)?;
        self.charge(insertion_work::<(u32, u32), bool>(self.reuse.reachability.len()))?;
        self.reuse.reachability.insert(key, result);
        Ok(result)
    }

    fn reaches_uncached(
        &mut self,
        from: u32,
        to: u32,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let mut pending = vec![from];
        let mut seen = BTreeSet::new();
        while let Some(block) = pending.pop() {
            self.charge(1)?;
            if !self.ssa.is_reachable(SsaBlockIdV1::new(block)) || !seen.insert(block) {
                continue;
            }
            if block == to {
                return Ok(true);
            }
            let body = self
                .body
                .blocks()
                .get(block as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            body.terminator()
                .kind()
                .try_for_each_edge::<ProductionSemanticKirErrorV1>(|edge| {
                    self.charge(1)?;
                    pending.push(edge.target().index());
                    Ok(())
                })?;
        }
        Ok(false)
    }

    pub(super) fn loan_live(
        &mut self,
        loan: CapabilityLoanV1,
        consumer: CapabilityDefinitionSiteV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let key = (loan, consumer);
        self.charge(lookup_work(self.reuse.loans.len()))?;
        if self.reuse.loans.contains_key(&key) {
            return Ok(());
        }
        self.loan_live_uncached(loan, consumer)?;
        self.charge(insertion_work::<
            (CapabilityLoanV1, CapabilityDefinitionSiteV1),
            (),
        >(self.reuse.loans.len()))?;
        self.reuse.loans.insert(key, ());
        Ok(())
    }

    fn loan_live_uncached(
        &mut self,
        loan: CapabilityLoanV1,
        consumer: CapabilityDefinitionSiteV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.use_value(loan.borrow.block, loan.owner_local)? != loan.owner_value {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        if !self.reaches(loan.borrow.block, consumer.block)? {
            return Err(reject("capability loan does not reach its consumer"));
        }
        // SSA establishes reaching definitions; the exact path intersection
        // retains every original storage invalidation and cycle check.
        let geometry = self.loan_region(loan.borrow.block, consumer.block)?;
        let region = &geometry.blocks;
        let acyclic = geometry.acyclic;
        let invalidations = self.owner_statement_index(loan.owner_local)?;
        for block in self.ssa.reverse_postorder() {
            self.charge(1)?;
            if !region[block.get() as usize] {
                continue;
            }
            let source = &self.body.blocks()[block.get() as usize];
            if !acyclic {
                let mut successors = Vec::new();
                source
                    .terminator()
                    .kind()
                    .try_for_each_edge::<ProductionSemanticKirErrorV1>(|edge| {
                        self.charge(1)?;
                        successors.push(edge.target().index());
                        Ok(())
                    })?;
                for successor in successors {
                    if self.reaches(successor, block.get())? {
                        return Err(reject(
                            "capability loan crosses a cycle without a proven storage generation",
                        ));
                    }
                }
            }
            let first = if block.get() == loan.borrow.block {
                loan.borrow.statement.map_or(0, |start| (start as usize).saturating_add(1))
            } else {
                0
            };
            let end = if block.get() == consumer.block {
                consumer.statement.map_or(source.statements().len(), |end| end as usize)
            } else {
                source.statements().len()
            };
            if self.indexed_statement_invalidated(&invalidations, block.get(), first, end)? {
                return Err(reject(
                    "capability loan crosses a move, overwrite, deinitialization or storage death",
                ));
            }
            if block.get() != consumer.block || consumer.statement.is_none() {
                if matches!(source.terminator().kind(), SemanticTerminatorKindV1::Drop { place, .. } if place.local().index() == loan.owner_local)
                {
                    return Err(reject("capability loan crosses an owner drop"));
                }
                if let SemanticTerminatorKindV1::Call(call) = source.terminator().kind() {
                    self.charge(call.arguments().len())?;
                    if call
                        .arguments()
                        .iter()
                        .any(|operand| moves(operand, loan.owner_local))
                        || call.destination().is_some_and(|destination| {
                            destination.place().local().index() == loan.owner_local
                        })
                    {
                        return Err(reject(
                            "capability loan crosses an owner call move or overwrite",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

pub(super) fn moves(operand: &SemanticOperandV1, local: u32) -> bool {
    matches!(operand, SemanticOperandV1::Move(place) if place.local().index() == local)
}

pub(super) fn invalidates(statement: &SemanticStatementKindV1, local: u32) -> bool {
    match statement {
        SemanticStatementKindV1::StorageLive(id) | SemanticStatementKindV1::StorageDead(id) => {
            id.index() == local
        }
        SemanticStatementKindV1::Deinitialize(place)
        | SemanticStatementKindV1::SetDiscriminant { place, .. } => place.local().index() == local,
        SemanticStatementKindV1::Assign(assignment) => {
            if assignment.destination().local().index() == local {
                return true;
            }
            match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand)
                | SemanticRvalueKindV1::Unary { operand, .. }
                | SemanticRvalueKindV1::Cast { operand, .. } => moves(operand, local),
                SemanticRvalueKindV1::Binary { left, right, .. } => {
                    moves(left, local) || moves(right, local)
                }
                SemanticRvalueKindV1::CheckedBinary(binary) => {
                    moves(binary.left(), local) || moves(binary.right(), local)
                }
                SemanticRvalueKindV1::UncheckedBinary(binary) => {
                    moves(binary.left(), local) || moves(binary.right(), local)
                }
                SemanticRvalueKindV1::Aggregate(aggregate) => aggregate
                    .operands()
                    .iter()
                    .any(|operand| moves(operand, local)),
                SemanticRvalueKindV1::Borrow { kind, place } => {
                    place.local().index() == local && *kind != SemanticBorrowKindV1::Shared
                }
                SemanticRvalueKindV1::AddressOf { place, .. } => place.local().index() == local,
                SemanticRvalueKindV1::Length(_)
                | SemanticRvalueKindV1::Discriminant(_)
                | SemanticRvalueKindV1::Load(_) => false,
            }
        }
        // These operations are not valid ways to mutate a tracked capability.
        SemanticStatementKindV1::Store(store) => {
            store.destination().local().index() == local || moves(store.value(), local)
        }
        SemanticStatementKindV1::AtomicRmw(atomic) => {
            atomic.address().local().index() == local
                || atomic.destination().local().index() == local
                || moves(atomic.value(), local)
        }
        SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
            atomic.address().local().index() == local
                || atomic.destination().local().index() == local
                || moves(atomic.expected(), local)
                || moves(atomic.replacement(), local)
        }
        SemanticStatementKindV1::Assume(operand) => moves(operand, local),
        SemanticStatementKindV1::Nop => false,
    }
}

fn reject(detail: &'static str) -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, detail)
}

include!("definition_lookup_v1.rs");

#[cfg(test)]
mod tests {
    use super::*;
    include!("tests.rs");
    include!("loan_reference.rs");
    include!("reuse_tests.rs");
    include!("invalidation_reference.rs");
    include!("reuse_invalidations.rs");
    include!("absent_use_observation_tests.rs");
    include!("work_failure_observation_tests.rs");
    include!("work_profile_graph_tests.rs");
    include!("region_acyclic_tests.rs");
    include!("region_acyclic_scratch_tests.rs");
    include!("region_acyclic_reference.rs");
    include!("definition_lookup_tests.rs");
    include!("definition_memo_tests.rs");
    include!("guard_selection_cost_tests124.rs");
    include!("source_plan_identity_tests.rs");
    include!("endpoint_scc_reference.rs");
    include!("endpoint_scc_tests.rs");
    include!("loan_region_reuse_tests.rs");
    include!("loan_region_reuse_reference.rs");
    include!("aggregate_invalidation_budget_tests.rs");
    include!("owner_invalidation_index_tests.rs");
    include!("owner_sparse_index_tests125.rs");
    include!("owner_invalidation_index_reference.rs");
    include!("path_region_scratch_tests.rs");
    include!("path_region_scratch_reference.rs");
    mod path_region_forward_cone_tests {
        use super::*;
        include!("path_region_forward_cone_tests.rs");
    }
}
