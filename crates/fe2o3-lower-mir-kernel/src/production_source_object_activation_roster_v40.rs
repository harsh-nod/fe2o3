// Original may-activation recipes retained in the existing immutable memory
// owner. Neither a static generation nor a chosen backing proves currentness.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingSourceObjectLifetimeV40 {
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    ty: SemanticTypeIdV1,
    slot: usize,
    activations: std::ops::Range<usize>,
}

struct PendingSourceObjectActivationsV40 {
    lifetimes: Vec<PendingSourceObjectLifetimeV40>,
    members: Vec<usize>,
    activations: Vec<SourceReferenceStorageActivationV29>,
}

fn object_activation_roster_headers_v40() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            std::mem::size_of::<T>(),
            argument_product_v1(
                2,
                std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>(),
            )?,
        ])
    }
    argument_sum_v1(&[
        h::<PendingSourceObjectActivationsV40>()?,
        h::<PendingSourceObjectLifetimeV40>()?,
        h::<SourceReferenceStorageActivationV29>()?,
        h::<Vec<PendingSourceObjectLifetimeV40>>()?,
        h::<Vec<usize>>()?,
        h::<Vec<SourceReferenceStorageActivationV29>>()?,
        h::<&SourceReferenceScalarCellV29>()?,
        h::<&SourceReferenceEpochSetV29>()?,
        h::<&[u32]>()?,
        h::<Option<usize>>()?,
        h::<(usize, u32, u32)>()?,
        h::<std::slice::Iter<'_, SourceReferenceScalarCellV29>>()?,
        h::<std::slice::Iter<'_, SourceReferenceStorageActivationV29>>()?,
        h::<std::slice::Iter<'_, u32>>()?,
        h::<std::slice::Windows<'_, PendingSourceObjectLifetimeV40>>()?,
        h::<([usize; 12], [&(); 10], [u32; 3])>()?,
        h::<()>()?,
    ])
}

fn object_epoch_members_v40<'a>(
    plan: &'a SourceReferencePlanV29<'_, '_>,
    row: &SourceReferenceScalarCellV29,
    limit: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<&'a [u32]>, ProductionSemanticKirErrorV1> {
    budget.charge_work(5)?;
    if row.generation < limit {
        return Ok(None);
    }
    let set = plan
        .epoch_sets
        .get((row.generation - limit) as usize)
        .ok_or_else(source_raw_physical_error_v29)?;
    if set.instance != row.instance
        || set.local != row.local
        || set.generation != row.generation
        || set.count < 2
    {
        return Err(source_raw_physical_error_v29());
    }
    let members = plan
        .epoch_members
        .get(set.first..argument_sum_v1(&[set.first, set.count])?)
        .ok_or_else(source_raw_physical_error_v29)?;
    let mut previous = None;
    for &atom in members {
        budget.charge_work(2)?;
        if atom >= limit || previous.is_some_and(|before| before >= atom) {
            return Err(source_raw_physical_error_v29());
        }
        previous = Some(atom);
    }
    Ok(Some(members))
}

fn capture_object_activations_v40(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    limits: &[u32],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PendingSourceObjectActivationsV40, ProductionSemanticKirErrorV1> {
    plan.check_owner(instances, budget)?;
    budget.reserve_storage(object_activation_roster_headers_v40()?)?;
    let mut count = 0usize;
    let mut member_count = 0usize;
    for row in &plan.cells.rows {
        budget.charge_work(1)?;
        if !matches!(row.kind, SourceBackingKindV29::Object(_)) {
            continue;
        }
        let limit = *limits
            .get(row.instance.index())
            .ok_or_else(source_raw_physical_error_v29)?;
        let members = object_epoch_members_v40(plan, row, limit, budget)?;
        count = argument_sum_v1(&[count, 1])?;
        member_count = argument_sum_v1(&[member_count, members.map_or(1, <[u32]>::len)])?;
    }
    if count == 0 {
        return Ok(PendingSourceObjectActivationsV40 {
            lifetimes: Vec::new(),
            members: Vec::new(),
            activations: Vec::new(),
        });
    }
    let mut lifetimes = emission_vec_v1(count, budget)?;
    let mut members = emission_vec_v1(member_count, budget)?;
    let mut activations = emission_vec_v1(plan.storage_activations.len(), budget)?;
    for &activation in &plan.storage_activations {
        budget.charge_work(1)?;
        activations.push(activation);
    }
    for row in &plan.cells.rows {
        budget.charge_work(2)?;
        if !matches!(row.kind, SourceBackingKindV29::Object(_)) {
            continue;
        }
        // Some source demands have no emitted backing. They cannot acquire a
        // recipe through this table; the eventual consumer must refuse absence.
        if !source_address_has_object_v29(
            plan,
            slots,
            row.instance,
            row.local,
            row.generation,
            budget,
        )? {
            continue;
        }
        let slot = source_address_object_slot_v29(
            instances,
            plan,
            slots,
            row.instance,
            row.local,
            row.generation,
            row.ty,
            budget,
        )?;
        let limit = *limits
            .get(row.instance.index())
            .ok_or_else(source_raw_physical_error_v29)?;
        let singleton = [row.generation];
        let atoms = object_epoch_members_v40(plan, row, limit, budget)?.unwrap_or(&singleton);
        let first = members.len();
        for &generation in atoms {
            budget.charge_work(4)?;
            let key = (row.instance.index(), row.local.index(), generation);
            charge_execution_cfg_lookup_v29(plan.storage_activation_sites.len(), budget)?;
            let &index = plan
                .storage_activation_sites
                .get(&key)
                .ok_or_else(source_raw_physical_error_v29)?;
            let activation = activations
                .get(index)
                .ok_or_else(source_raw_physical_error_v29)?;
            if (activation.instance, activation.local, activation.generation)
                != (row.instance, row.local, generation)
                || members.len() >= member_count
                || members.len() >= members.capacity()
            {
                return Err(source_raw_physical_error_v29());
            }
            members.push(index);
        }
        if first == members.len()
            || lifetimes.len() >= count
            || lifetimes.len() >= lifetimes.capacity()
        {
            return Err(source_raw_physical_error_v29());
        }
        lifetimes.push(PendingSourceObjectLifetimeV40 {
            instance: row.instance,
            local: row.local,
            generation: row.generation,
            ty: row.ty,
            slot,
            activations: first..members.len(),
        });
    }
    call_splice_sort_work_v1(lifetimes.len(), budget).map_err(source_address_call_error_v29)?;
    lifetimes.sort_unstable_by_key(|row| (row.instance.index(), row.local.index(), row.generation));
    for rows in lifetimes.windows(2) {
        budget.charge_work(1)?;
        if (
            rows[0].instance.index(),
            rows[0].local.index(),
            rows[0].generation,
        ) >= (
            rows[1].instance.index(),
            rows[1].local.index(),
            rows[1].generation,
        ) {
            return Err(source_raw_physical_error_v29());
        }
    }
    Ok(PendingSourceObjectActivationsV40 {
        lifetimes,
        members,
        activations,
    })
}

pub(super) type ObjectLifetimePartsV40<'a> = (
    SemanticTypeIdV1,
    usize,
    &'a [usize],
    &'a [SourceReferenceStorageActivationV29],
);

pub(super) fn object_lifetime_count_v40(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    relation.query(budget)?;
    budget.charge_work(1)?;
    Ok(relation
        .source
        .root_row(root)?
        .source_slots
        .pending_memory
        .as_ref()
        .map_or(0, |pending| pending.object_lifetimes.len()))
}

pub(super) fn object_lifetime_at_v40<'a>(
    relation: &'a ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    ordinal: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<((usize, SemanticLocalIdV1, u32), ObjectLifetimePartsV40<'a>)> {
    relation.query(budget)?;
    budget.charge_work(4)?;
    let pending = relation
        .source
        .root_row(root)?
        .source_slots
        .pending_memory
        .as_ref()
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "original object activation census absent",
        ))?;
    let row =
        pending
            .object_lifetimes
            .get(ordinal)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "original object lifetime ordinal",
            ))?;
    let members = pending
        .object_activation_members
        .get(row.activations.clone())
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "original object activation interval",
        ))?;
    if members.is_empty() {
        return relation
            .source
            .missing("original object has no activation members");
    }
    Ok((
        (row.instance.index(), row.local, row.generation),
        (row.ty, row.slot, members, &pending.object_activations),
    ))
}

// Source-only immutable recipe lookup. No checked access is manufactured from
// this table: exact-writer, currentness and active-view consumers remain intact.
pub(super) fn object_lifetime_parts_v40<'a>(
    relation: &'a ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    instance: usize,
    local: SemanticLocalIdV1,
    generation: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<ObjectLifetimePartsV40<'a>>> {
    relation.query(budget)?;
    relation.source.instance(root, instance, budget)?;
    budget.charge_work(1)?;
    let Some(pending) = relation
        .source
        .root_row(root)?
        .source_slots
        .pending_memory
        .as_ref()
    else {
        return Ok(None);
    };
    let key = (instance, local.index(), generation);
    let (mut lo, mut hi) = (0, pending.object_lifetimes.len());
    while lo < hi {
        budget.charge_work(1)?;
        let middle = lo + (hi - lo) / 2;
        let row = &pending.object_lifetimes[middle];
        if (row.instance.index(), row.local.index(), row.generation) < key {
            lo = middle + 1;
        } else {
            hi = middle;
        }
    }
    budget.charge_work(3)?;
    let Some(row) = pending
        .object_lifetimes
        .get(lo)
        .filter(|row| (row.instance.index(), row.local.index(), row.generation) == key)
    else {
        return Ok(None);
    };
    let members = pending
        .object_activation_members
        .get(row.activations.clone())
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "original object activation interval",
        ))?;
    if members.is_empty() {
        return relation
            .source
            .missing("original object has no activation members");
    }
    Ok(Some((
        row.ty,
        row.slot,
        members,
        &pending.object_activations,
    )))
}
