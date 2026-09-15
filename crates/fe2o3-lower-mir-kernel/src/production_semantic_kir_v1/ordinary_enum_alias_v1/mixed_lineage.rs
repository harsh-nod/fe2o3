//! Selected payload lineage through exact retained SSA values.
use super::*;
use fe2o3_pliron::{
    ProductionSemanticSsaIncomingValuesV1 as Incoming,
    ProductionSemanticSsaSourceQueryErrorV1 as QueryError,
    ProductionSemanticSsaSourceQueryV1 as Query, ProductionSemanticSsaSourceSiteV1 as Site,
    ProductionSemanticSsaValueOriginV1 as Origin, ProductionSemanticSsaValueV1 as Value,
};

type Key = (u32, SsaValueV1);
#[derive(Clone, Copy, Eq, PartialEq)]
enum ResultValue {
    Excluded,
    Slot(u32),
    Constructor(u32, Site),
}
enum Task<'a> {
    Enter(Value<'a>),
    Finish(Key),
    Capture {
        key: Key,
        source: u32,
        site: Site,
    },
    Merge {
        key: Key,
        incoming: Incoming<'a>,
        next: usize,
        result: ResultValue,
    },
}

pub(in super::super) struct MixedAliasPlan<'a> {
    query: Query<'a>,
    aggregate: bool,
    constructors: BTreeMap<(u32, u32), Option<Site>>,
    candidates: BTreeMap<u32, Alias>,
    audited: BTreeMap<u32, LocalAudit>,
    proofs: BTreeMap<(u32, u32), Option<ScalarEnumAliasV1>>,
    stable: BTreeMap<(u32, u32, u32), bool>,
    visited: Vec<bool>,
    queue: Vec<u32>,
}

pub(super) fn queried<T>(
    budget: &mut SemanticEnumAnalysisBudgetV1,
    f: impl FnOnce(&mut dyn FnMut() -> bool) -> Result<T, QueryError>,
) -> Result<Option<T>, ProductionSemanticKirErrorV1> {
    let mut failure = None;
    let result = f(&mut || match budget.charge_work(1) {
        Ok(()) => true,
        Err(error) => {
            failure = Some(error);
            false
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }
    // Query mismatches deny this optional certificate, never its budget error.
    Ok(result.ok())
}

fn push<'a>(
    tasks: &mut Vec<Task<'a>>,
    task: Task<'a>,
    budget: &mut SemanticEnumAnalysisBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    // Covers vector growth and the live frame, without refund on pop/miss.
    budget.charge_storage(
        2 * std::mem::size_of::<Task<'a>>().div_ceil(std::mem::size_of::<usize>()) + 4,
    )?;
    tasks.push(task);
    Ok(())
}

impl<'a> MixedAliasPlan<'a> {
    pub(in super::super) fn new(
        query: Query<'a>,
        types: &[SemanticTypeDeclV1],
        transport: &SemanticControlFlowSsaPlanV1,
        budget: &mut SemanticEnumAnalysisBudgetV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::new_kind(query, types, transport, budget, false)
    }

    pub(super) fn new_aggregate(
        query: Query<'a>,
        types: &[SemanticTypeDeclV1],
        transport: &SemanticControlFlowSsaPlanV1,
        budget: &mut SemanticEnumAnalysisBudgetV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::new_kind(query, types, transport, budget, true)
    }

    pub(in super::super) fn query(&self) -> Query<'a> {
        self.query
    }

    pub(super) fn constructor(
        &self,
        local: u32,
        variant: u32,
        budget: &mut SemanticEnumAnalysisBudgetV1,
    ) -> Result<Option<Site>, ProductionSemanticKirErrorV1> {
        budget.charge_work(lookup_work(self.constructors.len()))?;
        Ok(self.constructors.get(&(local, variant)).copied().flatten())
    }

    fn new_kind(
        query: Query<'a>,
        types: &[SemanticTypeDeclV1],
        transport: &SemanticControlFlowSsaPlanV1,
        budget: &mut SemanticEnumAnalysisBudgetV1,
        aggregate: bool,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let function = query.function();
        let mut candidates = BTreeMap::new();
        let mut audited = BTreeMap::new();
        let mut checked_types = BTreeMap::new();
        for (b, body) in function.blocks().iter().enumerate() {
            budget.charge_work(1)?;
            for (s, item) in body.statements().iter().enumerate() {
                budget.charge_work(1)?;
                let SemanticStatementKindV1::Assign(a) = item.kind() else {
                    continue;
                };
                let SemanticRvalueKindV1::Use(operand) = a.value().kind() else {
                    continue;
                };
                let Some(source) = whole_operand(operand) else {
                    continue;
                };
                let dest = a.destination();
                let ty = source.ty();
                if !dest.projections().is_empty()
                    || dest.ty() != ty
                    || a.value().result_type() != ty
                    || dest.local() == source.local()
                {
                    continue;
                }
                budget.charge_work(
                    3 * lookup_work(checked_types.len().saturating_add(1))
                        + lookup_work(transport.compiler_issued_bindings.len()),
                )?;
                if !checked_types.contains_key(&ty) {
                    let valid = !transport.compiler_issued_bindings.contains_key(&ty)
                        && if aggregate {
                            aggregate_custody::payload_type(types, transport, ty, budget)?.is_some()
                        } else {
                            scalar_enum(types, &transport.compiler_issued_bindings, ty, budget)?
                        };
                    budget.charge_storage(12)?;
                    checked_types.insert(ty, valid);
                }
                if !checked_types[&ty] {
                    continue;
                }
                let mut valid = true;
                for local in [dest.local(), source.local()] {
                    budget.charge_work(1 + lookup_work(transport.ssa_value_locals.len()))?;
                    valid &= function
                        .locals()
                        .get(local.index() as usize)
                        .is_some_and(|l| {
                            l.ty() == ty && l.role() == SemanticLocalRoleV1::Temporary
                        })
                        && transport.ssa_value_locals.contains(&local.index());
                }
                if !valid {
                    continue;
                }
                for local in [dest.local(), source.local()] {
                    budget.charge_work(2 * lookup_work(audited.len().saturating_add(1)))?;
                    if !audited.contains_key(&local.index()) {
                        budget.charge_storage(32)?;
                        audited.insert(
                            local.index(),
                            LocalAudit {
                                only_constructors: true,
                                ..Default::default()
                            },
                        );
                    }
                }
                budget.charge_work(
                    lookup_work(transport.promoted.len())
                        + 2 * lookup_work(candidates.len().saturating_add(1)),
                )?;
                if !transport.promoted.contains_key(&dest.local().index()) {
                    if !candidates.contains_key(&dest.local().index()) {
                        budget.charge_storage(16)?;
                    }
                    candidates.insert(
                        dest.local().index(),
                        Alias {
                            source: source.local().index(),
                            block: b as u32,
                            statement: s as u32,
                            ty,
                        },
                    );
                }
            }
        }
        audit_locals(types, function, &mut audited, budget)?;
        let mut constructors = BTreeMap::new();
        if aggregate {
            for (b, body) in function.blocks().iter().enumerate() {
                budget.charge_work(1)?;
                for (s, item) in body.statements().iter().enumerate() {
                    budget.charge_work(1 + lookup_work(audited.len()))?;
                    let SemanticStatementKindV1::Assign(a) = item.kind() else {
                        continue;
                    };
                    let local = a.destination().local().index();
                    if !audited.contains_key(&local) || !exact_constructor(types, a, budget)? {
                        continue;
                    }
                    let SemanticRvalueKindV1::Aggregate(a) = a.value().kind() else {
                        continue;
                    };
                    let SemanticAggregateKindV1::EnumVariant(v) = a.kind() else {
                        continue;
                    };
                    budget.charge_work(2 * lookup_work(constructors.len().saturating_add(1)))?;
                    let key = (local, *v);
                    if constructors.contains_key(&key) {
                        constructors.insert(key, None);
                    } else {
                        budget.charge_storage(16)?;
                        constructors.insert(
                            key,
                            Some(Site::new(
                                SemanticBlockIdV1::from_index(b as u32),
                                Some(s as u32),
                            )),
                        );
                    }
                }
            }
        }
        let count = function.blocks().len();
        budget.charge_work(count)?;
        budget.charge_storage(count.saturating_mul(2).saturating_add(28))?;
        Ok(Self {
            query,
            aggregate,
            constructors,
            candidates,
            audited,
            proofs: BTreeMap::new(),
            stable: BTreeMap::new(),
            visited: vec![false; count],
            queue: Vec::with_capacity(count),
        })
    }

    pub(in super::super) fn storage_owner(
        &mut self,
        types: &[SemanticTypeDeclV1],
        transport: &SemanticControlFlowSsaPlanV1,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        local: u32,
        variant: u32,
        budget: &mut SemanticEnumAnalysisBudgetV1,
    ) -> Result<Option<u32>, ProductionSemanticKirErrorV1> {
        budget.charge_work(2 * lookup_work(self.proofs.len().saturating_add(1)))?;
        if !self.proofs.contains_key(&(local, variant)) {
            let proof = self.prove(types, transport, local, variant, budget)?;
            budget.charge_work(lookup_work(self.proofs.len()))?;
            budget.charge_storage(16)?;
            self.proofs.insert((local, variant), proof);
        }
        let Some(proof) = self.proofs[&(local, variant)] else {
            return Ok(None);
        };
        Ok(proof
            .allows_use(
                self.query.function(),
                transport,
                block,
                statement,
                local,
                budget,
            )?
            .then_some(proof.source()))
    }

    fn prove(
        &mut self,
        types: &[SemanticTypeDeclV1],
        transport: &SemanticControlFlowSsaPlanV1,
        local: u32,
        variant: u32,
        budget: &mut SemanticEnumAnalysisBudgetV1,
    ) -> Result<Option<ScalarEnumAliasV1>, ProductionSemanticKirErrorV1> {
        budget.charge_work(lookup_work(self.candidates.len()) + lookup_work(self.audited.len()))?;
        let Some(&candidate) = self.candidates.get(&local) else {
            return Ok(None);
        };
        let audit = &self.audited[&local];
        if audit.invalid
            || audit.definitions != 1
            || audit.unique_site != Some((candidate.block, candidate.statement))
        {
            return Ok(None);
        }
        let site = Site::new(
            SemanticBlockIdV1::from_index(candidate.block),
            Some(candidate.statement),
        );
        budget.charge_work(lookup_work(transport.definition_values.len()))?;
        let Some([definition @ SsaValueV1::Definition(id)]) = transport
            .definition_values
            .get(&(candidate.block, local))
            .map(Vec::as_slice)
        else {
            return Ok(None);
        };
        let Some((variable, Origin::Event { site: actual, .. })) =
            queried(budget, |c| self.query.definition_origin(*id, &mut || c()))?
        else {
            return Ok(None);
        };
        if variable.get() != local || actual != site {
            return Ok(None);
        }
        let SemanticStatementKindV1::Assign(a) = self.query.function().blocks()
            [candidate.block as usize]
            .statements()[candidate.statement as usize]
            .kind()
        else {
            return Ok(None);
        };
        let SemanticRvalueKindV1::Use(operand) = a.value().kind() else {
            return Ok(None);
        };
        let Some(value) = queried(budget, |c| {
            self.query.operand_use(site, operand, &mut || c())
        })?
        else {
            return Ok(None);
        };
        let Some(source) = self.walk(
            types,
            transport,
            value.retained_value(),
            candidate.ty,
            variant,
            site,
            budget,
        )?
        else {
            return Ok(None);
        };
        Ok(Some(ScalarEnumAliasV1 {
            source,
            capture_block: candidate.block,
            capture_statement: candidate.statement,
            definition: *definition,
        }))
    }

    fn walk(
        &mut self,
        types: &[SemanticTypeDeclV1],
        transport: &SemanticControlFlowSsaPlanV1,
        value: Value<'a>,
        ty: SemanticTypeIdV1,
        variant: u32,
        capture: Site,
        budget: &mut SemanticEnumAnalysisBudgetV1,
    ) -> Result<Option<u32>, ProductionSemanticKirErrorV1> {
        let mut tasks = Vec::new();
        // None is an active node. A cycle is not resolved by optimistic caching.
        let mut cache = BTreeMap::<Key, Option<ResultValue>>::new();
        let mut last = ResultValue::Excluded;
        push(&mut tasks, Task::Enter(value), budget)?;
        while let Some(task) = tasks.pop() {
            budget.charge_work(1)?;
            match task {
                Task::Finish(key) => {
                    budget.charge_work(lookup_work(cache.len()))?;
                    cache.insert(key, Some(last));
                }
                Task::Capture { key, source, site } => {
                    if let ResultValue::Constructor(local, _) = last {
                        if local == source {
                            if !self.stable_slot(transport, local, ty, site, budget)? {
                                return Ok(None);
                            }
                            last = ResultValue::Slot(local);
                        }
                    }
                    budget.charge_work(lookup_work(cache.len()))?;
                    cache.insert(key, Some(last));
                }
                Task::Merge {
                    key,
                    incoming,
                    next,
                    mut result,
                } => {
                    if next != 0 {
                        result = match (result, last) {
                            (ResultValue::Excluded, other) | (other, ResultValue::Excluded) => {
                                other
                            }
                            (ResultValue::Slot(a), ResultValue::Slot(b)) if a == b => {
                                ResultValue::Slot(a)
                            }
                            (ResultValue::Constructor(a, x), ResultValue::Constructor(b, y))
                                if a == b && x == y =>
                            {
                                ResultValue::Constructor(a, x)
                            }
                            _ => return Ok(None),
                        };
                    }
                    if next == incoming.edge_count() {
                        budget.charge_work(lookup_work(cache.len()))?;
                        cache.insert(key, Some(result));
                        last = result;
                        continue;
                    }
                    let Some(Some((_edge, value))) =
                        queried(budget, |c| incoming.edge(next, &mut || c()))?
                    else {
                        return Ok(None);
                    };
                    push(
                        &mut tasks,
                        Task::Merge {
                            key,
                            incoming,
                            next: next + 1,
                            result,
                        },
                        budget,
                    )?;
                    push(&mut tasks, Task::Enter(value), budget)?;
                }
                Task::Enter(value) => {
                    let local = value.variable().get();
                    let key = (local, value.value());
                    budget.charge_work(
                        2 * lookup_work(cache.len().saturating_add(1))
                            + lookup_work(self.audited.len())
                            + 1,
                    )?;
                    if !value.belongs_to(&self.query)
                        || self
                            .query
                            .function()
                            .locals()
                            .get(local as usize)
                            .is_none_or(|l| {
                                l.ty() != ty || l.role() != SemanticLocalRoleV1::Temporary
                            })
                        || self.audited.get(&local).is_none_or(|a| a.invalid)
                    {
                        return Ok(None);
                    }
                    if let Some(result) = cache.get(&key) {
                        let Some(result) = result else {
                            return Ok(None);
                        };
                        last = *result;
                        continue;
                    }
                    budget.charge_storage(24)?;
                    cache.insert(key, None);
                    let Some(origin) =
                        queried(budget, |c| self.query.value_origin(&value, &mut || c()))?
                    else {
                        return Ok(None);
                    };
                    match origin {
                        Origin::Event { site, .. } => {
                            let Some(s) = site.statement() else {
                                return Ok(None);
                            };
                            let Some(item) = self
                                .query
                                .function()
                                .blocks()
                                .get(site.block().index() as usize)
                                .and_then(|b| b.statements().get(s as usize))
                            else {
                                return Ok(None);
                            };
                            let SemanticStatementKindV1::Assign(a) = item.kind() else {
                                return Ok(None);
                            };
                            if a.destination().local().index() != local
                                || !a.destination().projections().is_empty()
                                || a.destination().ty() != ty
                                || a.value().result_type() != ty
                            {
                                return Ok(None);
                            }
                            match a.value().kind() {
                                SemanticRvalueKindV1::Use(operand) => {
                                    let Some(place) = whole_operand(operand)
                                        .filter(|p| p.ty() == ty && p.local().index() != local)
                                    else {
                                        return Ok(None);
                                    };
                                    let Some(source) = queried(budget, |c| {
                                        self.query.operand_use(site, operand, &mut || c())
                                    })?
                                    else {
                                        return Ok(None);
                                    };
                                    if !self.aggregate
                                        && self.stable_slot(
                                            transport,
                                            place.local().index(),
                                            ty,
                                            site,
                                            budget,
                                        )?
                                    {
                                        last = ResultValue::Slot(place.local().index());
                                        budget.charge_work(lookup_work(cache.len()))?;
                                        cache.insert(key, Some(last));
                                    } else {
                                        push(
                                            &mut tasks,
                                            if self.aggregate {
                                                Task::Capture {
                                                    key,
                                                    source: place.local().index(),
                                                    site,
                                                }
                                            } else {
                                                Task::Finish(key)
                                            },
                                            budget,
                                        )?;
                                        push(
                                            &mut tasks,
                                            Task::Enter(source.retained_value()),
                                            budget,
                                        )?;
                                    }
                                }
                                SemanticRvalueKindV1::Aggregate(aggregate) => {
                                    let SemanticAggregateKindV1::EnumVariant(actual) =
                                        aggregate.kind()
                                    else {
                                        return Ok(None);
                                    };
                                    if (*actual == variant && !self.aggregate)
                                        || !exact_constructor(types, a, budget)?
                                    {
                                        return Ok(None);
                                    }
                                    last = if *actual != variant {
                                        ResultValue::Excluded
                                    } else if self.aggregate
                                        && self.constructor(local, variant, budget)? == Some(site)
                                    {
                                        ResultValue::Constructor(local, site)
                                    } else {
                                        return Ok(None);
                                    };
                                    budget.charge_work(lookup_work(cache.len()))?;
                                    cache.insert(key, Some(last));
                                }
                                _ => return Ok(None),
                            }
                        }
                        Origin::BlockArgument(incoming) => {
                            if !incoming.belongs_to(&self.query)
                                || incoming.external_entry().is_some()
                                || incoming.edge_count() == 0
                            {
                                return Ok(None);
                            }
                            push(
                                &mut tasks,
                                Task::Merge {
                                    key,
                                    incoming,
                                    next: 0,
                                    result: ResultValue::Excluded,
                                },
                                budget,
                            )?;
                        }
                        Origin::Entry { .. } | Origin::Edge { .. } => return Ok(None),
                    }
                }
            }
        }
        Ok(match last {
            ResultValue::Slot(local) => Some(local),
            ResultValue::Constructor(local, _)
                if self.aggregate
                    && value.variable().get() == local
                    && self.stable_slot(transport, local, ty, capture, budget)? =>
            {
                Some(local)
            }
            ResultValue::Constructor(_, _) => None,
            ResultValue::Excluded => None,
        })
    }

    fn stable_slot(
        &mut self,
        transport: &SemanticControlFlowSsaPlanV1,
        local: u32,
        ty: SemanticTypeIdV1,
        capture: Site,
        budget: &mut SemanticEnumAnalysisBudgetV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let Some(statement) = capture.statement() else {
            return Ok(false);
        };
        let block = capture.block().index();
        let key = (local, block, statement);
        budget.charge_work(
            lookup_work(self.stable.len())
                + lookup_work(transport.promoted.len())
                + lookup_work(self.audited.len()),
        )?;
        if let Some(&result) = self.stable.get(&key) {
            return Ok(result);
        }
        let Some(promoted) = transport.promoted.get(&local) else {
            return Ok(false);
        };
        let Some(audit) = self.audited.get(&local) else {
            return Ok(false);
        };
        if promoted.semantic_type != ty
            || promoted.transport_semantic_type != ty
            || !promoted.transport.uses_structural_enum_transport()
            || audit.invalid
            || (!self.aggregate && !audit.only_constructors)
            || audit.definitions == 0
            || audit
                .storage_dead
                .is_some_and(|(b, s)| b != block || s <= statement)
        {
            return Ok(false);
        }
        // Scalar slots retain their constructor-only audit. Aggregate slots also
        // reject alias writes reachable from capture, including loop backedges.
        // Later source death is still accepted only at this capture.
        budget.charge_work(self.visited.len())?;
        self.visited.fill(false);
        self.queue.clear();
        self.visited[block as usize] = true;
        self.queue.push(block);
        let mut cursor = 0;
        let mut stable = true;
        while cursor < self.queue.len() {
            let block = self.queue[cursor];
            cursor += 1;
            budget.charge_work(1 + lookup_work(audit.constructors.len()))?;
            if audit.constructors.contains(&block) {
                stable = false;
                break;
            }
            if self.aggregate {
                for item in self.query.function().blocks()[block as usize].statements() {
                    budget.charge_work(1)?;
                    if matches!(item.kind(), SemanticStatementKindV1::Assign(a) if a.destination().local().index() == local)
                    {
                        stable = false;
                        break;
                    }
                }
                if !stable {
                    break;
                }
            }
            self.query.function().blocks()[block as usize]
                .terminator()
                .kind()
                .try_for_each_edge(|edge| {
                    budget.charge_work(1)?;
                    let target = edge.target().index() as usize;
                    let Some(seen) = self.visited.get_mut(target) else {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    };
                    if !*seen {
                        *seen = true;
                        self.queue.push(target as u32);
                    }
                    Ok(())
                })?;
        }
        budget.charge_work(lookup_work(self.stable.len()))?;
        budget.charge_storage(16)?;
        self.stable.insert(key, stable);
        Ok(stable)
    }
}
