use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirControlFlowScopeErrorV1 as CfgError, ControlFlowLimits,
    with_canonical_kir_control_flow_v1,
};

#[path = "production_scalar_ssa_guard_indices_v1.rs"]
mod indices;
#[path = "production_scalar_ssa_guard_recipe_v1.rs"]
mod recipe;
#[cfg(test)]
#[path = "production_scalar_ssa_guard_consistency_v1_tests.rs"]
mod tests;
use indices::{Incoming, Indices};

impl From<CfgError> for Error {
    fn from(value: CfgError) -> Self {
        Self::Loops(CanonicalKirLoopErrorV1::from(value))
    }
}

pub(super) fn push<T>(rows: &mut Vec<T>, row: T, budget: &mut Budget<'_>) -> Result<()> {
    budget.charge_work(1)?;
    if rows.len() == rows.capacity() {
        return Err(Error::Mismatch("guard prepaid output capacity"));
    }
    rows.push(row);
    Ok(())
}

pub(super) fn require_inherited(
    owner: &ProductionScalarSsaEmissionOwnerV1,
    budget: &Budget<'_>,
) -> Result<usize> {
    let inherited_occurrences = if owner.captured_occurrences.is_none() {
        owner
            .original()
            .semantic_ssa()
            .occurrence_storage()
            .ok_or(Error::Mismatch("source occurrence receipt"))?
            .retained_storage()
    } else {
        0
    };
    let inherited = owner
        .retained_analysis_storage_v1()
        .checked_add(inherited_occurrences)
        .ok_or(Resource::Arithmetic)?;
    // Scratch reserved below must never stand in for a missing inherited receipt.
    if budget.storage() < inherited {
        return Err(Resource::Accounting.into());
    }
    Ok(inherited)
}

pub(super) fn derive<'s, R: GuardRequest>(
    owner: &'s ProductionScalarSsaEmissionOwnerV1,
    requests: &[R],
    limits: CanonicalKirLoopLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<(ProductionU32GuardReportV1<'s>, ProductionU32GuardStorageV1)> {
    require_inherited(owner, budget)?;
    budget.charge_work(1)?;
    if requests.len() > MAX_ROWS || requests.len() > limits.rows {
        return Err(Error::Limit);
    }
    // Check row-byte arithmetic before the first metadata allocation.
    table_bytes::<ProductionU32GuardRowV1<'s>>(requests.len())?;
    let floor = budget.storage();
    resources::scoped(budget, |budget| {
        budget.reserve_storage(size_of::<ProductionU32GuardReportV1<'s>>())?;
        budget.reserve_storage(
            size_of::<Vec<(u32, u32, usize)>>()
                .checked_add(size_of::<Vec<(FunctionCoordinate, usize)>>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut rows = Vec::new();
        let mut requests_index = Vec::new();
        let mut groups = Vec::new();
        reserve(&mut rows, requests.len(), budget)?;
        reserve(&mut requests_index, requests.len(), budget)?;
        reserve(&mut groups, requests.len(), budget)?;
        for request in requests {
            check_request(owner, *request, budget)?;
            push(
                &mut requests_index,
                (
                    request.root().index(),
                    request.function().index(),
                    request.ordinal(),
                ),
                budget,
            )?;
        }
        resources::sort_work(requests_index.len(), budget)?;
        requests_index.sort_unstable();
        budget.charge_work(requests_index.len())?;
        if requests_index.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::Mismatch("duplicate guard request"));
        }
        let indices = Indices::derive(owner, budget)?;
        let mut incoming = Incoming::prepare(owner, budget)?;
        owner.with_u32_recurrences_v1(limits, budget, |query, budget| {
            incoming.populate(query.inventory_for_guard(), budget)?;
            for request in requests {
                budget.charge_work(1)?;
                let outcome = match request.query(query, budget)? {
                    QueryOutcome::Unavailable(reason) => {
                        ProductionU32GuardConsistencyV1::Unavailable(
                            ProductionU32GuardUnavailableV1::Recurrence(reason),
                        )
                    }
                    QueryOutcome::Joined(fact) => recipe::check(
                        owner,
                        *request,
                        fact,
                        query.inventory_for_guard(),
                        &indices,
                        &incoming,
                        budget,
                    )?,
                };
                push(
                    &mut rows,
                    ProductionU32GuardRowV1 {
                        root: request.root(),
                        function: request.function(),
                        ordinal: request.ordinal(),
                        outcome,
                    },
                    budget,
                )?;
            }
            Ok(())
        })?;
        for (ordinal, row) in rows.iter().enumerate() {
            budget.charge_work(1)?;
            if let ProductionU32GuardConsistencyV1::Joined(fact) = row.outcome {
                push(&mut groups, (fact.body.function, ordinal), budget)?;
            }
        }
        resources::sort_work(groups.len(), budget)?;
        groups.sort_unstable();
        let mut start = 0;
        while start < groups.len() {
            budget.charge_work(1)?;
            let function = groups[start].0;
            let mut end = start + 1;
            while end < groups.len() {
                budget.charge_work(1)?;
                if groups[end].0 != function {
                    break;
                }
                end += 1;
            }
            with_canonical_kir_control_flow_v1(
                owner.original().executable(),
                function,
                ControlFlowLimits {
                    blocks: limits.blocks,
                    edges: limits.edges,
                    ..Default::default()
                },
                budget,
                |cfg, budget| -> Result<()> {
                    for (_, ordinal) in &groups[start..end] {
                        budget.charge_work(1)?;
                        let ProductionU32GuardConsistencyV1::Joined(fact) = rows[*ordinal].outcome
                        else {
                            return Err(Error::Mismatch("guard function grouping"));
                        };
                        check_control(owner, fact, cfg, budget)?;
                    }
                    Ok(())
                },
            )?;
            start = end;
        }
        let retained = size_of::<ProductionU32GuardReportV1<'s>>()
            .checked_add(table_bytes::<ProductionU32GuardRowV1<'s>>(rows.capacity())?)
            .ok_or(Resource::Arithmetic)?;
        if retained
            > budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?
        {
            return Err(Resource::Accounting.into());
        }
        let result = ProductionU32GuardReportV1 {
            owner,
            rows,
            retained,
        };
        Ok((result, ProductionU32GuardStorageV1(retained)))
    })
}

fn check_control(
    owner: &ProductionScalarSsaEmissionOwnerV1,
    fact: ProductionU32GuardConsistencyFactV1<'_>,
    cfg: &mut fe2o3_kernel_ir::CanonicalKirControlFlowViewV1<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(3)?;
    let Definition::Result { operation, .. } = fact.recurrence.update() else {
        return Err(Error::Mismatch("guard checked update definition"));
    };
    if !std::ptr::eq(cfg.owner(), owner.original().executable())
        || cfg.function() != fact.body.function
        || !cfg.is_reachable(fact.body, budget)?
        || !cfg.is_reachable(fact.exit, budget)?
        || !cfg.dominates(fact.then_edge.source, fact.body, budget)?
        || !cfg.dominates(fact.body, operation.block, budget)?
        || !cfg.dominates(fact.then_edge.source, operation.block, budget)?
    {
        return Err(Error::Mismatch("actual N guard control dominance"));
    }
    Ok(())
}

pub(super) fn check_request<R: GuardRequest>(
    owner: &ProductionScalarSsaEmissionOwnerV1,
    request: R,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let source = owner.original().semantic_ssa().source_semantic();
    charge_lookup(source.roots().len(), budget)?;
    budget.charge_work(8)?;
    let function = source
        .functions()
        .get(request.function().index() as usize)
        .ok_or(Error::Mismatch("guard report function"))?;
    if source.roots().binary_search(&request.root()).is_err()
        || request.semantic() != source.semantic_sha256()
        || request.function_identity() != function.identity()
        || request.grants_authority()
        || request.authorizes_compiler_transform()
    {
        return Err(Error::Mismatch("guard report actual source/root identity"));
    }
    let certificate = request
        .certificate()
        .ok_or(Error::Mismatch("guard report certificate ordinal"))?;
    if certificate.function() != request.function()
        || certificate.function_identity() != request.function_identity()
        || certificate.semantic_mir_sha256() != request.semantic()
    {
        return Err(Error::Mismatch("guard report certificate membership"));
    }
    charge_lookup(owner.emission.aliases.len(), budget)?;
    if owner
        .emission
        .aliases
        .binary_search_by_key(
            &(request.root().index(), request.function().index()),
            |row| row.key(),
        )
        .is_err()
    {
        return Err(Error::Mismatch("guard report actual C root/body alias"));
    }
    Ok(())
}
