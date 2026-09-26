// The solution is borrowed from one original call-instance/source owner. It is
// an early-header proof, not a producer, borrow, lifetime or storage permit.
struct ExecutionIdentityPlanV1<'a, 'source> {
    source: ExecutionCallSourceV29,
    index: ExecutionIdentitySourceIndexV1<'a, 'source>,
    references: &'a SourceReferencePlanV29<'a, 'source>,
    targets: Vec<(usize, u32)>,
    mapping: Vec<usize>,
    classes: Vec<usize>,
    returns: Vec<Option<ExecutionIdentityReturnSourceV1>>,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
}

fn execution_identity_targets_v1(
    instances: &ExecutionInstancesV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<(usize, u32)>, ProductionSemanticKirErrorV1> {
    let mut output = Vec::new();
    for (ordinal, row) in instances.instances().iter().enumerate() {
        budget.charge_work(2)?;
        let id = instances
            .id_at(ordinal)
            .ok_or_else(execution_identity_error_v1)?;
        budget.charge_work(1)?;
        if !instances.instance_reachable(id).ok_or_else(execution_identity_error_v1)? {
            continue;
        }
        let count = row.declaration().blocks().len();
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Vec<usize>>(),
            std::mem::size_of::<Vec<bool>>(),
        ])?)?;
        let mut positions = emission_vec_v1(count, budget)?;
        let mut late = emission_vec_v1(count, budget)?;
        budget.charge_work(argument_product_v1(count, 2)?)?;
        positions.resize(count, usize::MAX);
        late.resize(count, false);
        for (position, block) in row.ssa().plan().reverse_postorder().iter().enumerate() {
            budget.charge_work(1)?;
            if execution_identity_block_active_v1(instances, id, *block, budget)? {
                positions[block.get() as usize] = position;
            }
        }
        for edge in instances
            .occurrences(id)
            .ok_or_else(execution_identity_error_v1)?
            .successors()
        {
            budget.charge_work(2)?;
            if !execution_identity_successor_active_v1(
                instances, id, edge.id().source(), edge.edge().role(), budget,
            )? {
                continue;
            }
            let source = positions[edge.id().source().get() as usize];
            let target = edge.edge().target().index() as usize;
            if source != usize::MAX && source >= positions[target] {
                late[target] = true;
            }
        }
        for (block, late) in late.iter().copied().enumerate() {
            budget.charge_work(1)?;
            if !late {
                continue;
            }
            let live = row
                .ssa()
                .plan()
                .live_in(SsaBlockIdV1::new(block as u32))
                .ok_or_else(execution_identity_error_v1)?;
            let mut nominal = false;
            for variable in live {
                budget.charge_work(1)?;
                nominal |= execution_identity_channels_v1(
                    instances.owner().source_semantic().types(),
                    row.declaration().locals()[variable.get() as usize].ty(),
                    budget,
                )? != 0;
            }
            if nominal {
                emission_push_v1(&mut output, (ordinal, block as u32), budget)?;
            }
        }
        let scratch = argument_sum_v1(&[
            std::mem::size_of::<Vec<usize>>(),
            std::mem::size_of::<Vec<bool>>(),
            argument_product_v1(positions.capacity(), std::mem::size_of::<usize>())?,
            argument_product_v1(late.capacity(), std::mem::size_of::<bool>())?,
        ])?;
        drop((positions, late));
        budget.release_storage(scratch)?;
    }
    Ok(output)
}

#[cfg(test)]
std::thread_local! {
    static IDENTITY_CONSTRUCTION_FAULT_V1: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

fn with_execution_identity_plan_v1<'a, 'root, 'source, 'work, R>(
    instances: &'a ExecutionInstancesV29<'source>,
    references: &'a SourceReferencePlanV29<'a, 'source>,
    root: &source_storage_v29::SourceStorageRootV29<'a, 'root, 'source>,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: impl FnOnce(
        Option<&ExecutionIdentityPlanV1<'a, 'source>>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    // Construction completes before the consumer can reserve escaping output.
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let custody = root.custody_view();
    let growth_header = argument_sum_v1(&[
        std::mem::size_of::<source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>(),
        std::mem::size_of::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>(),
    ])?;
    let mut growth = None;
    let mut owned = 0;
    let mut required = floor;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let plan = (|| {
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of::<ExecutionIdentityPlanV1<'_, '_>>(),
                growth_header,
            ])?)?;
            required = budget.storage();
            if !references.retains_custody(instances, budget)
                || !root
                    .custody_view()
                    .retains_custody(instances, &references.failure, budget)
            {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            budget.charge_work(source_storage_v29::SOURCE_STORAGE_ROOT_GROWTH_WORK_V29)?;
            growth = Some(
                custody
                    .capture_retained_growth()
                    .ok_or(ArgumentResourceV1::Accounting)?,
            );
            #[cfg(test)]
            match IDENTITY_CONSTRUCTION_FAULT_V1.get() {
                1 => return Err(execution_identity_error_v1()),
                2 => std::panic::panic_any(1158usize),
                _ => {}
            }
            let targets = execution_identity_targets_v1(instances, budget)?;
            let returns_present = execution_identity_return_present_v1(instances, budget)?;
            let retained_present = execution_identity_retained_present_v1(instances, budget)?;
            if targets.is_empty() && !returns_present && !retained_present {
                drop(targets);
                budget.release_storage(std::mem::size_of::<ExecutionIdentityPlanV1<'_, '_>>())?;
                return Ok(None);
            }
            let mut index = ExecutionIdentitySourceIndexV1::new(instances, budget)?;
            index.add_retained_entries(references, root, budget)?;
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of::<Vec<usize>>(),
                std::mem::size_of::<ExecutionIdentityEquationsV1>(),
            ])?)?;
            let mut roots = Vec::new();
            for &row in index.retained.values() {
                budget.charge_work(1)?;
                emission_push_v1(&mut roots, row, budget)?;
            }
            for &(ordinal, block) in &targets {
                let instance = instances
                    .id_at(ordinal)
                    .ok_or_else(execution_identity_error_v1)?;
                let row = instances
                    .instance(instance)
                    .ok_or_else(execution_identity_error_v1)?;
                for variable in row
                    .ssa()
                    .plan()
                    .live_in(SsaBlockIdV1::new(block))
                    .ok_or_else(execution_identity_error_v1)?
                {
                    let local = SemanticLocalIdV1::from_index(variable.get());
                    let ty = row.declaration().locals()[variable.get() as usize].ty();
                    if execution_identity_channels_v1(
                        instances.owner().source_semantic().types(),
                        ty,
                        budget,
                    )? == 0
                    {
                        continue;
                    }
                    if execution_cfg_nominal_kind_v29(
                        instances.owner().source_semantic().types(),
                        ty
                    )?
                    .is_none()
                    {
                        execution_identity_aggregate_presence_v1(
                            instances, references, root, instance, block, local, budget,
                        )?;
                    }
                    charge_execution_cfg_lookup_v29(index.headers.len(), budget)?;
                    let header = *index
                        .headers
                        .get(&(ordinal, block, variable.get()))
                        .ok_or_else(execution_identity_error_v1)?;
                    emission_push_v1(&mut roots, header, budget)?;
                }
            }
            let returns = execution_identity_return_sources_v1(&index, &mut roots, budget)?;
            let (equations, mapping) = index.selected_equations(&roots, budget)?;
            let classes = equations.solve(budget)?;
            let scratch = argument_sum_v1(&[
                std::mem::size_of::<Vec<usize>>(),
                std::mem::size_of::<ExecutionIdentityEquationsV1>(),
                argument_product_v1(roots.capacity(), std::mem::size_of::<usize>())?,
                argument_product_v1(
                    equations.rows.capacity(),
                    std::mem::size_of::<ExecutionIdentityEquationV1>(),
                )?,
                argument_product_v1(equations.inputs.capacity(), std::mem::size_of::<usize>())?,
            ])?;
            drop((roots, equations));
            budget.release_storage(scratch)?;
            let source = ExecutionCallSourceV29::from_instances(instances, budget)?;
            let plan = ExecutionIdentityPlanV1 {
                source,
                index,
                references,
                targets,
                mapping,
                classes,
                returns,
                ledger: budget.work_ledger_identity_v1(),
                floor: budget.storage(),
            };
            Ok::<_, ProductionSemanticKirErrorV1>(Some(plan))
        })()?;
        owned = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        required = budget.storage();
        // Construction only borrows C2 today. If that changes, do not classify
        // retained source allocations as disposable identity-plan storage.
        budget.charge_work(4)?;
        if growth
            .as_ref()
            .is_none_or(|growth| !growth.permits_refund(floor, required, budget.storage(), owned))
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        consume(plan.as_ref(), budget)
    }));
    let refund = match &result {
        Ok(Ok(_)) => Some(owned),
        _ => budget.storage().checked_sub(floor),
    };
    let growth_permits = refund.is_some_and(|bytes| {
        growth
            .as_ref()
            .is_none_or(|growth| growth.permits_refund(floor, required, budget.storage(), bytes))
    });
    drop(growth);
    drop(custody);
    match result {
        Ok(Ok(value)) => {
            if budget.work_ledger_identity_v1() != ledger
                || budget.storage() < required
                || !growth_permits
                || !root.permits_cleanup_refund(instances, &references.failure, owned, budget)
            {
                root.deny_active_root_refund();
                return Err(ArgumentResourceV1::Accounting.into());
            }
            if let Err(error) = budget.release_storage(owned) {
                root.deny_active_root_refund();
                return Err(error.into());
            }
            Ok(value)
        }
        Ok(Err(error)) => {
            if budget.work_ledger_identity_v1() != ledger
                || budget.storage() < required
                || !growth_permits
                || !root.permits_cleanup_refund(
                    instances,
                    &references.failure,
                    budget.storage() - floor,
                    budget,
                )
                || budget.release_storage(budget.storage() - floor).is_err()
            {
                // A later cleanup refusal must not overwrite the callback error
                // or let the outer source scope refund this abandoned owner.
                root.deny_active_root_refund();
            }
            Err(error)
        }
        Err(payload) => {
            if budget.work_ledger_identity_v1() != ledger
                || budget.storage() < required
                || !growth_permits
                || !root.permits_cleanup_refund(
                    instances,
                    &references.failure,
                    budget.storage() - floor,
                    budget,
                )
                || budget.release_storage(budget.storage() - floor).is_err()
            {
                root.deny_active_root_refund();
            }
            std::panic::resume_unwind(payload)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn execution_identity_aggregate_presence_v1(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferencePlanV29<'_, '_>,
    root: &source_storage_v29::SourceStorageRootV29<'_, '_, '_>,
    instance: ProductionCallInstanceIdV1,
    block: u32,
    local: SemanticLocalIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if !references.retains_custody(instances, budget)
        || !root
            .custody_view()
            .retains_custody(instances, &references.failure, budget)
    {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.charge_work(references.blocks.len())?;
    let mut rows = references
        .blocks
        .iter()
        .filter(|row| row.instance == instance && row.block.index() == block);
    let state = rows.next().ok_or_else(execution_identity_error_v1)?.entry;
    if rows.next().is_some() {
        return Err(execution_identity_error_v1());
    }
    let storage = references
        .states
        .get(state)
        .and_then(|locals| locals.get(local.index() as usize))
        .and_then(|local| local.storage)
        .ok_or_else(|| {
            unsupported(
                0,
                None,
                None,
                "nominal aggregate loop needs original typed initialization evidence",
            )
        })?;
    let snapshot = *references
        .storage_snapshots
        .get(storage)
        .ok_or_else(execution_identity_error_v1)?;
    let initialized = root
        .snapshot_initialized(snapshot, &[], budget)
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
    Ok(())
}
