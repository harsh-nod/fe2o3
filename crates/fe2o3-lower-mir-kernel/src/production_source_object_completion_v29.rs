// Whole-cell completion shares the existing physical origin/history equations.
// These helpers authenticate source recipes; they never seed physical origins.
include!("production_source_static_object_source_v29.rs");
include!("production_source_array_component_index_v29.rs");
include!("production_source_static_raw_holders_v42.rs");
fn source_address_local_slot_ranges_v29(
    slots: &[ScopedSourceSlotV29],
    local: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(Option<usize>, std::ops::Range<usize>), ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_product_v1(
        3,
        call_splice_search_work_v1(slots.len()),
    )?)?;
    let legacy = slots
        .binary_search_by_key(&ScopedAllocationIdentityV29::LegacyLocal(local), |row| {
            row.origin.identity
        })
        .ok();
    let first = ScopedAllocationIdentityV29::OriginalObject {
        local,
        generation: 0,
    };
    let last = ScopedAllocationIdentityV29::OriginalObject {
        local,
        generation: u32::MAX,
    };
    let start = slots.partition_point(|row| row.origin.identity < first);
    let end = slots.partition_point(|row| row.origin.identity <= last);
    Ok((legacy, start..end))
}

fn source_address_has_object_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let physical_generation = plan
        .physical_object_generation(instance, local, generation, budget)?
        .map_or(generation, |(_, _, row)| row.generation);
    budget.charge_work(call_splice_search_work_v1(slots.instances.len()))?;
    let Some(owner) = slots
        .instances
        .binary_search_by_key(&instance.index(), |row| row.instance.index())
        .ok()
        .and_then(|index| slots.instances.get(index))
    else {
        return Ok(false);
    };
    let rows = slots
        .slots
        .get(owner.slots.clone())
        .ok_or_else(source_raw_physical_error_v29)?;
    budget.charge_work(call_splice_search_work_v1(rows.len()))?;
    Ok(rows
        .binary_search_by_key(
            &ScopedAllocationIdentityV29::OriginalObject {
                local: local.index(),
                generation: physical_generation,
            },
            |row| row.origin.identity,
        )
        .is_ok())
}

fn source_address_object_slot_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    ty: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    plan.check_owner(instances, budget)?;
    if slots.ledger != budget.work_ledger_identity_v1()
        || slots.source != ExecutionCallSourceV29::from_instances(instances, budget)?
    {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let original = instances
        .instance(instance)
        .ok_or_else(source_raw_physical_error_v29)?;
    budget.charge_work(4)?;
    if original
        .declaration()
        .locals()
        .get(local.index() as usize)
        .map(|row| row.ty())
        != Some(ty)
    {
        return Err(source_raw_physical_error_v29());
    }
    budget.charge_work(call_splice_search_work_v1(slots.instances.len()))?;
    let owner = slots
        .instances
        .binary_search_by_key(&instance.index(), |row| row.instance.index())
        .ok()
        .and_then(|index| slots.instances.get(index))
        .ok_or_else(source_raw_physical_error_v29)?;
    let rows = slots
        .slots
        .get(owner.slots.clone())
        .ok_or_else(source_raw_physical_error_v29)?;
    budget.charge_work(call_splice_search_work_v1(rows.len()))?;
    let mapping = plan.physical_object_generation(instance, local, generation, budget)?;
    let identity = ScopedAllocationIdentityV29::OriginalObject {
        local: local.index(),
        generation: mapping.map_or(generation, |(_, _, row)| row.generation),
    };
    let offset = rows
        .binary_search_by_key(&identity, |row| row.origin.identity)
        .map_err(|_| source_raw_physical_error_v29())?;
    let row = &rows[offset];
    let (
        ScopedAllocationSourceV29::OriginalObject {
            cell,
            schema: original_schema,
        },
        ScopedSlotRepresentationV29::Object {
            schema,
            bytes,
            alignment,
        },
    ) = (row.origin.source, row.representation)
    else {
        return Err(source_raw_physical_error_v29());
    };
    if row.instance != instance
        || owner.function != original.function()
        || row.origin.semantic_type != ty
        || original_schema != schema
    {
        return Err(source_raw_physical_error_v29());
    }
    let logical_cell = if let Some((logical, representative, physical)) = mapping {
        if representative != cell
            || physical.ty != ty
            || physical.kind != SourceBackingKindV29::Object(schema)
        {
            return Err(source_raw_physical_error_v29());
        }
        logical
    } else {
        cell
    };
    if !budget.source_object_storage_matches_v29(
        plan,
        logical_cell,
        instance,
        local,
        generation,
        schema,
        Some((bytes, alignment)),
    )? {
        return Err(source_raw_physical_error_v29());
    }
    argument_sum_v1(&[owner.slots.start, offset]).map_err(Into::into)
}

fn source_address_object_payload_v29(
    anchors: &ScopedMemoryAnchorsV29,
    row: &ScopedMemoryAnchorV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<(ScopedObjectEndpointV29, ScopedMemoryPayloadV29)>, ProductionSemanticKirErrorV1>
{
    if !matches!(row.kind, ScopedMemoryAnchorKindV29::Object(_)) {
        return Ok(None);
    }
    let payload = anchors.object_payload(row, budget)?;
    if matches!(
        payload.operation,
        ScopedObjectOperationV29::ReadDiscriminant { .. }
            | ScopedObjectOperationV29::SetDiscriminant { .. }
            | ScopedObjectOperationV29::Project {
                step: ScopedObjectProjectionV29::Field(_)
                    | ScopedObjectProjectionV29::VariantForWrite { .. },
                ..
            }
    ) {
        // These rows have no scalar value payload. Tags are authenticated by
        // the complete original tag census and independent byte-range graph;
        // constructor views retain their selected source schema and geometry.
        return Ok(None);
    }
    budget.charge_work(8)?;
    let (endpoint, role) = match (payload.operation, payload.role) {
        (
            ScopedObjectOperationV29::Project {
                step: ScopedObjectProjectionV29::ArrayIndex(_),
                ..
            },
            _,
        ) => {
            // Project is not a value access. The complete source census joins
            // its numeric producer; geometry and cell history remain mandatory.
            return Ok(None);
        }
        (
            ScopedObjectOperationV29::ReadValue { .. },
            ScopedObjectRoleV29::ReadValue {
                source,
                read: ScopedObjectReadOriginV29::ProjectionIndex(read),
            },
        ) => (
            source,
            ScopedMemoryPayloadV29::IndexLoad {
                result: payload.result.ok_or_else(scoped_object_error_v29)?,
                read,
            },
        ),
        (
            ScopedObjectOperationV29::ReadValue { .. },
            ScopedObjectRoleV29::ReadValue {
                source,
                read: ScopedObjectReadOriginV29::Original(read),
            },
        ) => (
            source,
            ScopedMemoryPayloadV29::Load {
                result: payload.result.ok_or_else(scoped_object_error_v29)?,
                read,
            },
        ),
        (
            ScopedObjectOperationV29::WriteValue { value, .. },
            ScopedObjectRoleV29::WriteValue {
                destination,
                value: ScopedObjectValueOriginV29::Original(source),
            },
        ) => (destination, ScopedMemoryPayloadV29::Store { value, source }),
        _ => return Err(scoped_object_pending_v29()),
    };
    match (endpoint.object, endpoint.source) {
        (
            ScopedObjectIdentityV29::Local {
                local,
                generation: 0,
                ..
            },
            ScopedObjectSourceV29::EntryArgument { local: original },
        ) if local == original
            && endpoint.source_path == (ScopedObjectPathV29 { first: 0, count: 0 })
            && endpoint.path == endpoint.source_path
            && endpoint.root_type == endpoint.projected_type
            && endpoint.root_schema == endpoint.projected_schema
            && matches!(role, ScopedMemoryPayloadV29::Store {
                    source: ScopedMemoryStoreSourceV29::EntryArgument { local: actual, ty }, ..
                } if actual == local && ty == endpoint.root_type) => {}
        (
            ScopedObjectIdentityV29::Local { local, .. },
            ScopedObjectSourceV29::ProjectionIndex(read),
        ) if local == read.local
            && endpoint.source_path.count == 0
            && endpoint.path.count == 0
            && endpoint.root_type == read.ty
            && endpoint.projected_type == read.ty
            && endpoint.root_schema == endpoint.projected_schema
            && matches!(role, ScopedMemoryPayloadV29::IndexLoad { read: actual, .. } if actual == read) =>
            {}
        (ScopedObjectIdentityV29::Local { .. }, ScopedObjectSourceV29::Place { .. }) => {
            if endpoint.source_path.count != endpoint.path.count {
                return Err(scoped_object_pending_v29());
            }
            for component in anchors.object_path(endpoint.source_path, budget)? {
                budget.charge_work(1)?;
                if !matches!(component, ScopedObjectComponentV29::Original { projection, selector: None }
                    if matches!(projection.kind(), SemanticProjectionKindV1::Field(_)
                        | SemanticProjectionKindV1::ConstantIndex { .. }))
                {
                    return Err(scoped_object_pending_v29());
                }
            }
        }
        (
            ScopedObjectIdentityV29::Local { .. },
            ScopedObjectSourceV29::AggregateComponent { variant, .. },
        ) if matches!(
            role,
            ScopedMemoryPayloadV29::Store {
                source: ScopedMemoryStoreSourceV29::Operand { .. },
                ..
            }
        ) && endpoint.source_path.count == 0
            && endpoint.path.count == if variant.is_some() { 2 } else { 1 } => {}
        (
            ScopedObjectIdentityV29::Reference {
                dereference_prefix, ..
            },
            ScopedObjectSourceV29::Place { prefix, .. },
        ) if endpoint.root_type == endpoint.projected_type
            && endpoint.root_schema == endpoint.projected_schema
            && endpoint.path.count == 0
            && prefix == dereference_prefix
            && prefix != 0 =>
        {
            let path = anchors.object_path(endpoint.source_path, budget)?;
            budget.charge_work(path.len())?;
            if path.len() != prefix as usize || path.iter().enumerate().any(|(index, component)| {
                    !matches!(component, ScopedObjectComponentV29::Original { projection, selector: None }
                        if if index + 1 == path.len() {
                            projection.kind() == SemanticProjectionKindV1::Dereference
                        } else { matches!(projection.kind(), SemanticProjectionKindV1::Field(_)
                            | SemanticProjectionKindV1::ConstantIndex { .. }) })
                }) { return Err(scoped_object_pending_v29()); }
        }
        (
            ScopedObjectIdentityV29::Reference {
                dereference_prefix, ..
            },
            ScopedObjectSourceV29::Place { prefix, .. },
        ) if dereference_prefix != 0
            && prefix.checked_sub(dereference_prefix) == Some(1)
            && endpoint.path.count == 1 =>
        {
            let path = anchors.object_path(endpoint.source_path, budget)?;
            budget.charge_work(path.len())?;
            if path.len() != prefix as usize || path.iter().enumerate().any(|(index, component)| {
                    !matches!(component, ScopedObjectComponentV29::Original { projection, selector: None }
                        if if index + 1 == dereference_prefix as usize {
                            projection.kind() == SemanticProjectionKindV1::Dereference
                        } else { matches!(projection.kind(), SemanticProjectionKindV1::Field(_)) })
                }) { return Err(scoped_object_pending_v29()); }
        }
        _ => return Err(scoped_object_pending_v29()),
    }
    Ok(Some((endpoint, role)))
}

fn source_address_object_entry_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    instance: ProductionCallInstanceIdV1,
    endpoint: ScopedObjectEndpointV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    plan.check_owner(instances, budget)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<usize>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&ScopedSourceSlotV29>>(plan, budget)?;
        let local = source_object_entry_local_v29(plan, instance, endpoint, budget)?;
        let slot = source_address_object_slot_v29(
            instances,
            plan,
            slots,
            instance,
            local,
            0,
            endpoint.root_type,
            budget,
        )?;
        budget.charge_work(3)?;
        if !matches!(slots.slots.get(slot),
        Some(ScopedSourceSlotV29 { representation: ScopedSlotRepresentationV29::Object { schema, .. }, .. })
            if *schema == endpoint.root_schema)
        {
            return Err(scoped_object_error_v29());
        }
        Ok(slot)
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_address_object_index_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    instance: ProductionCallInstanceIdV1,
    read: ScopedMemoryIndexReadV29,
    endpoint: ScopedObjectEndpointV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    source_reference_owned_prepay_v29::<usize>(plan, budget)?;
    source_reference_owned_prepay_v29::<Option<&ScopedSourceSlotV29>>(plan, budget)?;
    let generation = source_retained_index_generation_v29(plan, instance, read, budget)?;
    budget.charge_work(8)?;
    if endpoint.object
        != (ScopedObjectIdentityV29::Local {
            instance,
            local: read.local,
            generation,
        })
        || endpoint.source != ScopedObjectSourceV29::ProjectionIndex(read)
        || endpoint.root_type != read.ty
        || endpoint.projected_type != read.ty
        || endpoint.root_schema != endpoint.projected_schema
        || endpoint.source_path.count != 0
        || endpoint.path.count != 0
    {
        return Err(source_raw_physical_error_v29());
    }
    let slot = source_address_object_slot_v29(
        instances, plan, slots, instance, read.local, generation, read.ty, budget,
    )?;
    if !matches!(slots.slots.get(slot),
        Some(ScopedSourceSlotV29 { representation: ScopedSlotRepresentationV29::Object { schema, .. }, .. })
            if *schema == endpoint.root_schema)
    {
        return Err(source_raw_physical_error_v29());
    }
    Ok(slot)
}

fn source_address_object_direct_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    site: SourceReferenceSiteV29,
    place: &SemanticPlaceV1,
    access: SourceReferenceAccessV29,
    endpoint: ScopedObjectEndpointV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    let ScopedObjectIdentityV29::Local {
        instance,
        local,
        generation,
    } = endpoint.object
    else {
        return Err(source_raw_physical_error_v29());
    };
    if instance != site.instance || local != place.local() {
        return Err(source_raw_physical_error_v29());
    }
    budget.charge_work(place.projections().len())?;
    if place.projections().iter().all(|projection| {
        matches!(
            projection.kind(),
            SemanticProjectionKindV1::Field(_) | SemanticProjectionKindV1::ConstantIndex { .. }
        )
    }) {
        let original = source_reference_access_at_v29(plan, site, place, access, budget)?;
        let projections = plan
            .projections
            .get(original.projections.clone())
            .ok_or_else(source_raw_physical_error_v29)?;
        budget.charge_work(place.projections().len())?;
        if original.instance != instance
            || original.local != local
            || original.generation != generation
            || original.ty != place.ty()
            || original.loan.is_some()
            || original.shared_path
            || projections != place.projections()
            || !original.traversed.is_empty()
        {
            return Err(source_raw_physical_error_v29());
        }
    } else {
        let crossing = place
            .projections()
            .len()
            .checked_sub(1)
            .ok_or_else(source_raw_physical_error_v29)?;
        let raw = if crossing == 0 {
            let key = (source_reference_access_key_v29(site, place, access), 0);
            charge_execution_cfg_lookup_v29(plan.raw_accesses.len(), budget)?;
            **plan
                .raw_accesses
                .get(&key)
                .ok_or_else(source_raw_physical_error_v29)?
        } else {
            source_static_raw_holder_v42(plan, site, place, access, crossing, budget)?
        };
        let holder = raw.holder;
        if raw.site != site
            || raw.source != place as *const SemanticPlaceV1 as usize
            || raw.access != access
            || holder.instance != instance
            || holder.local != local
            || holder.generation != generation
            || (crossing == 0
                && (holder.count != 0
                    || holder.parent.is_some()
                    || holder.shared_path
                    || holder.selector_source.is_some()))
            || !matches!(endpoint.source, ScopedObjectSourceV29::Place { prefix, .. }
                if prefix as usize <= crossing)
        {
            return Err(source_raw_physical_error_v29());
        }
    }
    let slot = source_address_object_slot_v29(
        instances,
        plan,
        slots,
        instance,
        local,
        generation,
        endpoint.root_type,
        budget,
    )?;
    if !matches!(slots.slots[slot].representation, ScopedSlotRepresentationV29::Object { schema, .. }
        if schema == endpoint.root_schema)
    {
        return Err(source_raw_physical_error_v29());
    }
    Ok(slot)
}

fn source_address_live_generation_v29(
    function: &SemanticFunctionDeclV1,
    site: ExecutionSiteV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<u32, ProductionSemanticKirErrorV1> {
    let (block, statement) = scoped_memory_site_key_v29(site);
    let statement = statement.ok_or_else(source_raw_physical_error_v29)? as usize;
    let mut generation = 1usize;
    for (ordinal, source) in function.blocks().iter().enumerate() {
        budget.charge_work(1)?;
        if ordinal == block as usize {
            if statement >= source.statements().len() {
                return Err(source_raw_physical_error_v29());
            }
            return u32::try_from(argument_sum_v1(&[generation, statement])?)
                .map_err(|_| ArgumentResourceV1::Arithmetic.into());
        }
        generation = argument_sum_v1(&[generation, source.statements().len()])?;
    }
    Err(source_raw_physical_error_v29())
}

#[cfg(test)]
type SourceObjectEffectCensusObserverV29 = fn(
    &SourceAddressSourceIndexV29<'_>,
    &mut Vec<SourceAddressAccessSourceV29>,
    &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1>;

#[cfg(test)]
thread_local! {
    static SOURCE_OBJECT_EFFECT_CENSUS_OBSERVER_V29:
        std::cell::Cell<Option<SourceObjectEffectCensusObserverV29>> =
        const { std::cell::Cell::new(None) };
}

fn check_source_object_effect_census_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    sources: &[SourceAddressAccessSourceV29],
    failures: &[SourceIndexFailureV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    plan.check_owner(instances, budget)?;
    if slots.ledger != budget.work_ledger_identity_v1()
        || slots.source != ExecutionCallSourceV29::from_instances(instances, budget)?
    {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    #[cfg(test)]
    let candidate_sources = if let Some(observer) = SOURCE_OBJECT_EFFECT_CENSUS_OBSERVER_V29.get() {
        source_reference_owned_prepay_v29::<Vec<SourceAddressAccessSourceV29>>(plan, budget)?;
        let mut rows = emission_vec_v1(argument_sum_v1(&[sources.len(), 1])?, budget)?;
        budget.charge_work(sources.len())?;
        rows.extend_from_slice(sources);
        observer(source_index, &mut rows, budget)?;
        Some(rows)
    } else {
        None
    };
    #[cfg(test)]
    let sources = candidate_sources.as_deref().unwrap_or(sources);
    source_reference_owned_prepay_v29::<(
        Vec<bool>,
        Vec<bool>,
        Vec<bool>,
        Vec<bool>,
        Vec<(usize, u32)>,
    )>(plan, budget)?;
    source_reference_owned_prepay_v29::<Option<(usize, SourceReferenceSelectorV29)>>(plan, budget)?;
    source_reference_owned_prepay_v29::<Option<&SemanticPlaceV1>>(plan, budget)?;
    source_reference_owned_prepay_v29::<Option<&mut bool>>(plan, budget)?;
    source_reference_owned_prepay_v29::<ScopedEmittedPointsV29<'_, '_, '_>>(plan, budget)?;
    source_reference_owned_prepay_v29::<Result<Option<(BlockId, u32)>, ScopedTileFailureKindV29>>(
        plan, budget,
    )?;
    source_reference_owned_prepay_v29::<(usize, usize, Option<(usize, u32)>)>(plan, budget)?;
    source_reference_owned_prepay_v29::<(u32, u32)>(plan, budget)?;
    let mut entry_count = 0usize;
    let mut previous_entry = None;
    let mut selector_reads = emission_vec_v1(plan.selectors.len(), budget)?;
    budget.charge_work(plan.selectors.len())?;
    selector_reads.resize(plan.selectors.len(), false);
    let mut terminal = emission_vec_v1(plan.accesses.len(), budget)?;
    let mut tags = emission_vec_v1(plan.accesses.len(), budget)?;
    budget.reserve_storage(source_address_tag_query_headers_v43()?)?;
    let mut holders = emission_vec_v1(plan.raw_accesses.len(), budget)?;
    let mut aggregate_fields = Vec::new();
    budget.charge_work(argument_sum_v1(&[
        plan.accesses.len(),
        plan.accesses.len(),
        plan.raw_accesses.len(),
    ])?)?;
    terminal.resize(plan.accesses.len(), false);
    tags.resize(plan.accesses.len(), false);
    holders.resize(plan.raw_accesses.len(), false);
    for source in sources {
        let sidecar = source_index.sidecar(source.instance, budget)?;
        let anchors = sidecar
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(scoped_object_error_v29)?;
        let row = anchors
            .rows
            .get(source.anchor)
            .ok_or_else(scoped_object_error_v29)?;
        if let Some((index, demand)) = source_address_tag_effect_v43(
            instances,
            plan,
            source_index,
            slots,
            source,
            anchors,
            row,
            budget,
        )? {
            budget.charge_work(2)?;
            let seen = tags.get_mut(index).ok_or_else(scoped_object_error_v29)?;
            if std::mem::replace(seen, true) {
                return Err(scoped_object_error_v29());
            }
            if demand == SourceTagDemandV43::Statement {
                let seen = terminal
                    .get_mut(index)
                    .ok_or_else(scoped_object_error_v29)?;
                if std::mem::replace(seen, true) {
                    return Err(scoped_object_error_v29());
                }
            }
            continue;
        }
        let Some((endpoint, payload)) = source_address_object_payload_v29(anchors, row, budget)?
        else {
            continue;
        };
        budget.charge_work(2)?;
        if let ScopedObjectSourceV29::EntryArgument { local } = endpoint.source {
            budget.charge_work(5)?;
            let key = (source.instance.index(), local.index());
            if previous_entry.is_some_and(|previous| previous >= key)
                || row.source.is_some()
                || source.raw.is_some()
                || source_address_object_entry_v29(
                    instances,
                    plan,
                    slots,
                    source.instance,
                    endpoint,
                    budget,
                )? != source.physical.slot
                || !matches!(payload, ScopedMemoryPayloadV29::Store {
                    source: ScopedMemoryStoreSourceV29::EntryArgument { local: actual, ty }, ..
                } if actual == local && ty == endpoint.root_type)
            {
                return Err(scoped_object_error_v29());
            }
            let mut mapping = ScopedEmittedPointsV29 {
                coordinates: &source_index.pending.coordinates,
                relocation: &source_index.pending.slot_relocation,
                budget,
            };
            let physical = mapping
                .emitted_point(
                    source.instance,
                    row.block,
                    u32::try_from(row.position).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    false,
                )
                .map_err(source_address_point_error_v29)?
                .ok_or_else(scoped_object_error_v29)?;
            if physical
                != (
                    source.physical.block,
                    u32::try_from(source.physical.operation)
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                )
            {
                return Err(scoped_object_error_v29());
            }
            previous_entry = Some(key);
            entry_count = argument_sum_v1(&[entry_count, 1])?;
            continue;
        }
        if let ScopedObjectSourceV29::ProjectionIndex(read) = endpoint.source {
            // Implicit selectors are not explicit Place accesses. Rejoin their
            // original use and physical row, then count each selector once.
            if !matches!(payload, ScopedMemoryPayloadV29::IndexLoad { read: actual, .. } if actual == read)
                || source.raw.is_some()
                || row.source != Some(ScopedMemoryFrameV29::operand(read.site, Some(read.role)))
                || source_address_object_index_v29(
                    instances,
                    plan,
                    slots,
                    source.instance,
                    read,
                    endpoint,
                    budget,
                )? != source.physical.slot
            {
                return Err(scoped_object_error_v29());
            }
            let original = instances
                .instance(source.instance)
                .ok_or_else(scoped_object_error_v29)?
                .declaration();
            let place = source_reference_selector_place_v29(original, read.site, read.role)
                .ok_or_else(scoped_object_error_v29)?;
            let (index, selector) = plan
                .selector_at(
                    source.instance,
                    read.site,
                    place,
                    read.projection as usize,
                    budget,
                )?
                .ok_or_else(scoped_object_error_v29)?;
            budget.charge_work(5)?;
            if selector.local != read.local
                || selector.value
                    != (SourceReferenceSelectorValueV29::Retained { event: read.event })
            {
                return Err(scoped_object_error_v29());
            }
            source_index.frame_gap(
                source.instance,
                ScopedMemoryFrameV29::operand(read.site, Some(read.role)),
                row.block,
                row.position,
                budget,
            )?;
            let mut mapping = ScopedEmittedPointsV29 {
                coordinates: &source_index.pending.coordinates,
                relocation: &source_index.pending.slot_relocation,
                budget,
            };
            let physical = mapping
                .emitted_point(
                    source.instance,
                    row.block,
                    u32::try_from(row.position).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    false,
                )
                .map_err(source_address_point_error_v29)?
                .ok_or_else(scoped_object_error_v29)?;
            if physical
                != (
                    source.physical.block,
                    u32::try_from(source.physical.operation)
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                )
            {
                return Err(scoped_object_error_v29());
            }
            let seen = selector_reads
                .get_mut(index)
                .ok_or_else(scoped_object_error_v29)?;
            if std::mem::replace(seen, true) {
                return Err(source_reference_error_v29(
                    "original typed selector read is duplicated in the actual candidate",
                ));
            }
            continue;
        }
        if let ScopedObjectSourceV29::AggregateComponent {
            site,
            operand,
            destination,
            variant: _,
        } = endpoint.source
        {
            let original = instances
                .instance(source.instance)
                .ok_or_else(scoped_object_error_v29)?
                .declaration();
            let Some(SemanticStatementKindV1::Assign(assignment)) =
                scoped_source_statement_v29(original, site)
            else {
                return Err(scoped_object_error_v29());
            };
            if assignment.destination().local() != destination
                || !assignment.destination().projections().is_empty()
                || !matches!(payload, ScopedMemoryPayloadV29::Store { .. })
            {
                return Err(scoped_object_error_v29());
            }
            let (block, statement) = scoped_memory_site_key_v29(site);
            let key = source_reference_access_key_v29(
                SourceReferenceSiteV29 {
                    instance: source.instance,
                    block: SemanticBlockIdV1::from_index(block),
                    statement: statement.map(|row| row as usize),
                },
                assignment.destination(),
                SourceReferenceAccessV29::Write,
            );
            charge_execution_cfg_lookup_v29(plan.access_sites.len(), budget)?;
            let ordinal = *plan
                .access_sites
                .get(&key)
                .ok_or_else(scoped_object_error_v29)?;
            scoped_object_reserve_append_v29(&mut aggregate_fields, budget)?;
            aggregate_fields.push((ordinal, operand));
            continue;
        }
        let ScopedObjectSourceV29::Place {
            site, role, prefix, ..
        } = endpoint.source
        else {
            return Err(scoped_object_pending_v29());
        };
        let function = instances
            .instance(source.instance)
            .ok_or_else(scoped_object_error_v29)?
            .declaration();
        let place = scoped_object_original_place_v29(function, site, role)
            .ok_or_else(scoped_object_error_v29)?;
        let (block, statement) = scoped_memory_site_key_v29(site);
        let site = SourceReferenceSiteV29 {
            instance: source.instance,
            block: SemanticBlockIdV1::from_index(block),
            statement: statement.map(|value| value as usize),
        };
        let access = source_reference_raw_original_access_v29(function, site, place, budget)?
            .ok_or_else(scoped_object_error_v29)?;
        let key = source_reference_access_key_v29(site, place, access);
        let writing = matches!(payload, ScopedMemoryPayloadV29::Store { .. });
        if prefix as usize == place.projections().len() {
            if access
                != if writing {
                    SourceReferenceAccessV29::Write
                } else {
                    SourceReferenceAccessV29::Read
                }
            {
                return Err(scoped_object_error_v29());
            }
            charge_execution_cfg_lookup_v29(plan.access_sites.len(), budget)?;
            let &index = plan
                .access_sites
                .get(&key)
                .ok_or_else(scoped_object_error_v29)?;
            let seen = terminal
                .get_mut(index)
                .ok_or_else(scoped_object_error_v29)?;
            if std::mem::replace(seen, true) {
                return Err(scoped_object_error_v29());
            }
        } else {
            if writing
                || place
                    .projections()
                    .get(prefix as usize)
                    .map(|projection| projection.kind())
                    != Some(SemanticProjectionKindV1::Dereference)
            {
                return Err(scoped_object_pending_v29());
            }
            charge_execution_cfg_lookup_v29(plan.raw_accesses.len(), budget)?;
            let raw = plan
                .raw_accesses
                .get(&(key, prefix as usize))
                .ok_or_else(scoped_object_error_v29)?;
            let seen = holders
                .get_mut(raw.ordinal)
                .ok_or_else(scoped_object_error_v29)?;
            if std::mem::replace(seen, true) {
                return Err(scoped_object_error_v29());
            }
        }
    }
    let mut expected_entries = 0usize;
    for instance in &slots.instances {
        budget.charge_work(2)?;
        let rows = slots
            .slots
            .get(instance.slots.clone())
            .ok_or_else(scoped_object_error_v29)?;
        // This is the same original declaration/slot roster used before
        // relocation, not a count inferred from actual entry stores.
        let Some(first) = rows.first() else {
            continue;
        };
        let entry = first.allocation.block;
        visit_scoped_slot_initializers_v29(
            instances,
            instance.instance,
            entry,
            rows,
            budget,
            |_, slot, _, _| {
                if matches!(
                    slot.representation,
                    ScopedSlotRepresentationV29::Object { .. }
                ) {
                    expected_entries = argument_sum_v1(&[expected_entries, 1])?;
                }
                Ok(())
            },
        )?;
    }
    if entry_count != expected_entries {
        return Err(scoped_object_error_v29());
    }
    for (index, selector) in plan.selectors.iter().enumerate() {
        budget.charge_work(4)?;
        selector.check(instances, budget)?;
        let expected = match (selector.value, selector.retained_generation) {
            (SourceReferenceSelectorValueV29::Retained { .. }, Some(generation)) => {
                source_address_has_object_v29(
                    plan,
                    slots,
                    selector.instance,
                    selector.local,
                    generation,
                    budget,
                )?
            }
            (SourceReferenceSelectorValueV29::Retained { .. }, None)
                if plan.storage_root.is_some() =>
            {
                return Err(scoped_memory_error_v29());
            }
            _ => false,
        };
        if selector_reads[index] != expected {
            return Err(source_reference_error_v29(
                "original typed selector read is missing from the actual candidate",
            ));
        }
    }
    for failure in failures {
        budget.charge_work(4)?;
        let original = instances
            .instance(failure.instance)
            .ok_or_else(scoped_object_error_v29)?
            .declaration();
        let occurrences = instances
            .occurrences(failure.instance)
            .ok_or_else(scoped_object_error_v29)?;
        let anchors = source_index
            .sidecar(failure.instance, budget)?
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(scoped_object_error_v29)?;
        let row = anchors
            .rows
            .get(failure.anchor)
            .ok_or_else(scoped_object_error_v29)?;
        let place = checked_scoped_failure_read_v29(original, &occurrences, row, budget)?;
        let frame = row.source.ok_or_else(scoped_object_error_v29)?;
        let (block, statement) = scoped_memory_site_key_v29(frame.site);
        let site = SourceReferenceSiteV29 {
            instance: failure.instance,
            block: SemanticBlockIdV1::from_index(block),
            statement: statement.map(|row| row as usize),
        };
        if source_failure_scalar_slot_v29(instances, plan, slots, site, place, budget)?
            != failure.slot
            || source_failure_operand_moved_v29(original, row, budget)? != failure.move_after
        {
            return Err(scoped_object_error_v29());
        }
        charge_execution_cfg_lookup_v29(plan.access_sites.len(), budget)?;
        let ordinal = *plan
            .access_sites
            .get(&source_reference_access_key_v29(
                site,
                place,
                SourceReferenceAccessV29::Read,
            ))
            .ok_or_else(scoped_object_error_v29)?;
        let seen = terminal
            .get_mut(ordinal)
            .ok_or_else(scoped_object_error_v29)?;
        if std::mem::replace(seen, true) {
            return Err(scoped_object_error_v29());
        }
    }
    call_splice_sort_work_v1(argument_product_v1(aggregate_fields.len(), 2)?, budget)
        .map_err(source_address_call_error_v29)?;
    aggregate_fields.sort_unstable();
    check_source_grid_leader_zero_census_v29(
        instances,
        plan,
        source_index,
        slots,
        &mut terminal,
        budget,
    )?;
    let mut next_aggregate_field = 0;
    for (index, access) in plan.accesses.iter().enumerate() {
        budget.charge_work(4)?;
        let object = source_address_has_object_v29(
            plan,
            slots,
            access.instance,
            access.local,
            access.generation,
            budget,
        )?;
        if !object
            || !matches!(
                access.key.access,
                SourceReferenceAccessV29::Read
                    | SourceReferenceAccessV29::Write
                    | SourceReferenceAccessV29::ReadDiscriminant
            )
        {
            continue;
        }
        let function = instances
            .instance(access.key.site.instance)
            .ok_or_else(scoped_object_error_v29)?
            .declaration();
        if tags[index]
            != source_address_original_tag_demand_v43(function, access.key, budget)?.is_some()
        {
            return Err(source_reference_error_v29(
                "original typed tag effect is missing or unrelated to its source occurrence",
            ));
        }
        let original = access.key.site.statement.and_then(|ordinal| {
            function
                .blocks()
                .get(access.key.site.block.index() as usize)
                .and_then(|block| block.statements().get(ordinal))
        });
        if let Some(SemanticStatementKindV1::Deinitialize(place)) =
            original.map(|statement| statement.kind())
        {
            if place as *const SemanticPlaceV1 as usize == access.key.source {
                if !place.projections().is_empty() {
                    return Err(scoped_object_pending_v29());
                }
                continue;
            }
        }
        if let Some(SemanticStatementKindV1::Assign(assignment)) =
            original.map(|statement| statement.kind())
            && assignment.destination() as *const SemanticPlaceV1 as usize == access.key.source
            && assignment.destination().projections().is_empty()
            && access.key.access == SourceReferenceAccessV29::Write
            && let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind()
        {
            // This original-demand census is independent of the actual Project
            // list, so deleting all generated operations cannot select a weaker lane.
            let types = instances.owner().source_semantic().types();
            let declaration = types
                .get(assignment.destination().ty().index() as usize)
                .ok_or_else(scoped_object_error_v29)?;
            let field_count = if let (
                SemanticTypeShapeV1::Array { element, length },
                SemanticAggregateKindV1::Array,
            ) = (declaration.shape(), aggregate.kind())
            {
                budget.charge_work(6)?;
                require_ordinary_execution_representation_v29(declaration)?;
                let count = usize::try_from(*length).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                let element_type = types
                    .get(element.index() as usize)
                    .ok_or_else(scoped_object_error_v29)?;
                require_ordinary_execution_representation_v29(element_type)?;
                if count == 0
                    || count > MAX_SSA_VALUE_COMPONENTS_V1
                    || count != aggregate.operands().len()
                    || !matches!(
                        element_type.shape(),
                        SemanticTypeShapeV1::Scalar(_)
                            | SemanticTypeShapeV1::ValidityScalar(_)
                            | SemanticTypeShapeV1::Pointer(_)
                    )
                {
                    return Err(scoped_object_pending_v29());
                }
                for operand in aggregate.operands() {
                    budget.charge_work(2)?;
                    if operand.ty() != *element {
                        return Err(scoped_object_error_v29());
                    }
                }
                count
            } else if let SemanticAggregateKindV1::EnumVariant(variant) = aggregate.kind() {
                source_object_enum_field_types_v43(
                    types,
                    assignment.destination().ty(),
                    *variant,
                    aggregate.operands(),
                    budget,
                )?
                .len()
            } else {
                source_object_aggregate_field_types_v29(
                    types,
                    assignment.destination().ty(),
                    aggregate.kind(),
                    aggregate.operands().len(),
                    budget,
                )?
                .len()
            };
            if terminal[index] {
                return Err(scoped_object_error_v29());
            }
            for operand in 0..field_count {
                budget.charge_work(2)?;
                if aggregate_fields.get(next_aggregate_field)
                    != Some(&(
                        index,
                        u32::try_from(operand).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    ))
                {
                    return Err(source_reference_error_v29(
                        "original typed aggregate field effect is missing or duplicated",
                    ));
                }
                next_aggregate_field += 1;
            }
            terminal[index] = true;
        }
        if !terminal[index] {
            return Err(source_reference_error_v29(
                "original typed value effect is missing from the actual candidate",
            ));
        }
    }
    if next_aggregate_field != aggregate_fields.len() {
        return Err(scoped_object_error_v29());
    }
    for raw in plan.raw_accesses.values() {
        budget.charge_work(4)?;
        let holder = raw.holder;
        let object = source_address_has_object_v29(
            plan,
            slots,
            holder.instance,
            holder.local,
            holder.generation,
            budget,
        )?;
        if object && !holders.get(raw.ordinal).copied().unwrap_or(false) {
            return Err(source_reference_error_v29(
                "original typed pointer holder read is missing from the actual candidate",
            ));
        }
    }
    let bytes = argument_sum_v1(&[
        terminal.capacity(),
        tags.capacity(),
        holders.capacity(),
        selector_reads.capacity(),
        argument_product_v1(
            aggregate_fields.capacity(),
            std::mem::size_of::<(usize, u32)>(),
        )?,
    ])?;
    drop((terminal, tags, holders, selector_reads, aggregate_fields));
    budget.release_storage(bytes)?;
    #[cfg(test)]
    if let Some(rows) = candidate_sources {
        let bytes = argument_product_v1(
            rows.capacity(),
            std::mem::size_of::<SourceAddressAccessSourceV29>(),
        )?;
        drop(rows);
        budget.release_storage(bytes)?;
    }
    Ok(())
}

type SourceObjectOccurrenceKeyV29 = (usize, [u64; 7], u32);
type SourceObjectReadKeyV29 = (usize, [u64; 7], u32, usize);

struct SourceObjectPayloadIndexV29 {
    pending: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    occurrences: Vec<(SourceObjectOccurrenceKeyV29, usize)>,
    reads: Vec<(SourceObjectReadKeyV29, usize)>,
    projects: Vec<((usize, ValueId), usize)>,
}

fn source_object_read_key_v29(
    instance: ProductionCallInstanceIdV1,
    read: ScopedMemoryReadV29,
) -> SourceObjectReadKeyV29 {
    let event = match read.occurrence {
        ScopedMemoryOccurrenceV29::Promoted { event, .. }
        | ScopedMemoryOccurrenceV29::Retained { event } => event,
    };
    (
        instance.index(),
        unit_local_source_key_v1(read.site, read.role, Some(ExecutionEventV29::BaseUse)),
        read.prefix,
        event,
    )
}

#[cfg(test)]
type SourceObjectPayloadIndexObserverV29 = fn(
    &mut SourceObjectPayloadIndexV29,
    &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1>;

#[cfg(test)]
thread_local! {
    static SOURCE_OBJECT_PAYLOAD_INDEX_OBSERVER_V29: std::cell::Cell<Option<SourceObjectPayloadIndexObserverV29>> = const { std::cell::Cell::new(None) };
    static SOURCE_OBJECT_PAYLOAD_INDEX_WORK_V29: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    static SOURCE_OBJECT_PAYLOAD_PASS_WORK_V29: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    static SOURCE_OBJECT_PAYLOAD_CONSTRUCTION_WORK_V29: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    static SOURCE_OBJECT_PROJECT_INDEX_WORK_V29: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    static SOURCE_OBJECT_PROJECT_INDEX_STORAGE_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SOURCE_OBJECT_PROJECT_CENSUS_WORK_V29: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    static SOURCE_OBJECT_PAYLOAD_SCAN_COUNTS_V29: std::cell::Cell<(usize, usize, usize)> = const { std::cell::Cell::new((0, 0, 0)) };
    static SOURCE_OBJECT_BASE_USE_CENSUS_WORK_V29: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    static SOURCE_OBJECT_BASE_USE_FIRST_SCAN_V29: std::cell::Cell<Option<(usize, usize)>> = const { std::cell::Cell::new(None) };
    static SOURCE_OBJECT_BASE_USE_STORAGE_V29: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    static SOURCE_OBJECT_READ_INDEX_STORAGE_V29: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    static SOURCE_OBJECT_READ_CENSUS_WORK_V29: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    static SOURCE_OBJECT_READ_FIRST_CLASSIFICATION_V29: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn source_object_record_index_work_v29(before: usize, budget: &ArgumentBudgetV1<'_>) {
    let (calls, work) = SOURCE_OBJECT_PAYLOAD_INDEX_WORK_V29.get();
    SOURCE_OBJECT_PAYLOAD_INDEX_WORK_V29.set((
        calls.checked_add(1).unwrap(),
        work.checked_add(budget.work() - before).unwrap(),
    ));
}

impl SourceObjectPayloadIndexV29 {
    fn new(
        instances: &ExecutionInstancesV29<'_>,
        source_index: &SourceAddressSourceIndexV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        #[cfg(test)]
        let before = budget.work();
        source_reference_emission_prepay_v29::<Self>(budget)?;
        source_reference_emission_prepay_v29::<Vec<((usize, ValueId), usize)>>(budget)?;
        source_reference_emission_prepay_v29::<((usize, ValueId), usize)>(budget)?;
        source_reference_emission_prepay_v29::<(usize, ValueId)>(budget)?;
        source_reference_emission_prepay_v29::<(usize, usize, usize)>(budget)?;
        source_reference_emission_prepay_v29::<Option<ValueId>>(budget)?;
        source_reference_emission_prepay_v29::<std::slice::Windows<'_, ((usize, ValueId), usize)>>(
            budget,
        )?;
        source_reference_emission_prepay_v29::<Option<&[((usize, ValueId), usize)]>>(budget)?;
        source_reference_emission_prepay_v29::<&[((usize, ValueId), usize)]>(budget)?;
        source_reference_emission_prepay_v29::<&ScopedObjectPayloadV29>(budget)?;
        source_reference_emission_prepay_v29::<ValueId>(budget)?;
        source_reference_emission_prepay_v29::<usize>(budget)?;
        let (mut event_count, mut read_count, mut project_count) = (0, 0, 0);
        for sidecar in &source_index.pending.sidecars.rows {
            budget.charge_work(3)?;
            let anchors = sidecar
                .scoped_memory_anchors
                .as_ref()
                .ok_or_else(scoped_object_error_v29)?;
            let instance = sidecar
                .source_call_instance
                .ok_or_else(scoped_object_error_v29)?;
            if anchors.objects.is_empty()
                && !source_scalar_static_holder_index_needed_v29(
                    instances, instance, anchors, budget,
                )?
            {
                continue;
            }
            let occurrences = instances
                .occurrences(instance)
                .ok_or_else(scoped_object_error_v29)?;
            // The index stores BaseUse rows, not every original occurrence.
            #[cfg(test)]
            let before_base_use_scan = budget.work();
            #[cfg(test)]
            if SOURCE_OBJECT_BASE_USE_FIRST_SCAN_V29.get().is_none() {
                SOURCE_OBJECT_BASE_USE_FIRST_SCAN_V29
                    .set(Some((budget.work(), occurrences.events().len())));
            }
            budget.charge_work(occurrences.events().len())?;
            for row in occurrences.events() {
                #[cfg(test)]
                {
                    let (count, fill, events) = SOURCE_OBJECT_PAYLOAD_SCAN_COUNTS_V29.get();
                    SOURCE_OBJECT_PAYLOAD_SCAN_COUNTS_V29.set((count, fill, events + 1));
                }
                if row.role() == ExecutionEventV29::BaseUse {
                    event_count = argument_sum_v1(&[event_count, 1])?;
                }
            }
            #[cfg(test)]
            {
                let (scan, final_check) = SOURCE_OBJECT_BASE_USE_CENSUS_WORK_V29.get();
                SOURCE_OBJECT_BASE_USE_CENSUS_WORK_V29
                    .set((scan + budget.work() - before_base_use_scan, final_check));
            }
            for row in &anchors.rows {
                #[cfg(test)]
                {
                    let (count, fill, events) = SOURCE_OBJECT_PAYLOAD_SCAN_COUNTS_V29.get();
                    SOURCE_OBJECT_PAYLOAD_SCAN_COUNTS_V29.set((count + 1, fill, events));
                }
                budget.charge_work(1)?;
                if matches!(
                    row.kind,
                    ScopedMemoryAnchorKindV29::Access {
                        payload: Some(ScopedMemoryPayloadV29::Load { .. }),
                        ..
                    }
                ) {
                    read_count = argument_sum_v1(&[read_count, 1])?;
                }
                if matches!(row.kind, ScopedMemoryAnchorKindV29::Object(_)) {
                    let payload = anchors.object_payload(row, budget)?;
                    budget.charge_work(1)?;
                    if matches!(payload.operation, ScopedObjectOperationV29::Project { .. }) {
                        project_count = argument_sum_v1(&[project_count, 1])?;
                    }
                    // Capacity counts only original reads. The fill still
                    // validates the complete payload, endpoint and source path.
                    #[cfg(test)]
                    if SOURCE_OBJECT_READ_FIRST_CLASSIFICATION_V29.get().is_none() {
                        SOURCE_OBJECT_READ_FIRST_CLASSIFICATION_V29.set(Some(budget.work()));
                    }
                    budget.charge_work(2)?;
                    #[cfg(test)]
                    {
                        let (classify, final_check) = SOURCE_OBJECT_READ_CENSUS_WORK_V29.get();
                        SOURCE_OBJECT_READ_CENSUS_WORK_V29.set((classify + 2, final_check));
                    }
                    if matches!(
                        (payload.operation, payload.role),
                        (
                            ScopedObjectOperationV29::ReadValue { .. },
                            ScopedObjectRoleV29::ReadValue {
                                read: ScopedObjectReadOriginV29::Original(_),
                                ..
                            }
                        )
                    ) {
                        read_count = argument_sum_v1(&[read_count, 1])?;
                    }
                }
            }
        }
        let mut result = Self {
            pending: source_index.pending as *const PendingScopedRootEmissionV29 as usize,
            ledger: budget.work_ledger_identity_v1(),
            occurrences: {
                #[cfg(test)]
                let before = budget.storage();
                let rows = emission_vec_v1(event_count, budget)?;
                #[cfg(test)]
                {
                    let (calls, bytes) = SOURCE_OBJECT_BASE_USE_STORAGE_V29.get();
                    SOURCE_OBJECT_BASE_USE_STORAGE_V29
                        .set((calls + 1, bytes + budget.storage() - before));
                }
                rows
            },
            reads: {
                #[cfg(test)]
                let before = budget.storage();
                let rows = emission_vec_v1(read_count, budget)?;
                #[cfg(test)]
                {
                    let (calls, bytes) = SOURCE_OBJECT_READ_INDEX_STORAGE_V29.get();
                    SOURCE_OBJECT_READ_INDEX_STORAGE_V29
                        .set((calls + 1, bytes + budget.storage() - before));
                }
                rows
            },
            projects: emission_vec_v1(project_count, budget)?,
        };
        for sidecar in &source_index.pending.sidecars.rows {
            budget.charge_work(3)?;
            let anchors = sidecar
                .scoped_memory_anchors
                .as_ref()
                .ok_or_else(scoped_object_error_v29)?;
            let instance = sidecar
                .source_call_instance
                .ok_or_else(scoped_object_error_v29)?;
            if anchors.objects.is_empty()
                && !source_scalar_static_holder_index_needed_v29(
                    instances, instance, anchors, budget,
                )?
            {
                continue;
            }
            let occurrences = instances
                .occurrences(instance)
                .ok_or_else(scoped_object_error_v29)?;
            for (event, row) in occurrences.events().iter().enumerate() {
                #[cfg(test)]
                {
                    let (count, fill, events) = SOURCE_OBJECT_PAYLOAD_SCAN_COUNTS_V29.get();
                    SOURCE_OBJECT_PAYLOAD_SCAN_COUNTS_V29.set((count, fill, events + 1));
                }
                budget.charge_work(12)?;
                if row.role() != ExecutionEventV29::BaseUse {
                    continue;
                }
                if result.occurrences.len() == result.occurrences.capacity() {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                result.occurrences.push((
                    (
                        instance.index(),
                        unit_local_source_key_v1(row.site(), row.operand(), Some(row.role())),
                        row.event().variable().get(),
                    ),
                    event,
                ));
            }
            for (anchor, row) in anchors.rows.iter().enumerate() {
                #[cfg(test)]
                {
                    let (count, fill, events) = SOURCE_OBJECT_PAYLOAD_SCAN_COUNTS_V29.get();
                    SOURCE_OBJECT_PAYLOAD_SCAN_COUNTS_V29.set((count, fill + 1, events));
                }
                budget.charge_work(12)?;
                if matches!(row.kind, ScopedMemoryAnchorKindV29::Object(_)) {
                    let payload = anchors.object_payload(row, budget)?;
                    budget.charge_work(3)?;
                    if matches!(payload.operation, ScopedObjectOperationV29::Project { .. }) {
                        if result.projects.len() == result.projects.capacity() {
                            return Err(ArgumentResourceV1::Accounting.into());
                        }
                        result.projects.push((
                            (
                                instance.index(),
                                payload.result.ok_or_else(scoped_object_error_v29)?,
                            ),
                            anchor,
                        ));
                    }
                }
                if let Some((_, read)) = source_object_read_payload_v29(anchors, row, budget)? {
                    if result.reads.len() == result.reads.capacity() {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    result
                        .reads
                        .push((source_object_read_key_v29(instance, read), anchor));
                }
            }
        }
        call_splice_sort_work_v1(argument_product_v1(result.occurrences.len(), 9)?, budget)
            .map_err(source_address_call_error_v29)?;
        result.occurrences.sort_unstable_by_key(|row| row.0);
        call_splice_sort_work_v1(argument_product_v1(result.reads.len(), 10)?, budget)
            .map_err(source_address_call_error_v29)?;
        result.reads.sort_unstable_by_key(|row| row.0);
        call_splice_sort_work_v1(argument_product_v1(result.projects.len(), 3)?, budget)
            .map_err(source_address_call_error_v29)?;
        result.projects.sort_unstable_by_key(|row| row.0);
        #[cfg(test)]
        if let Some(observe) = SOURCE_OBJECT_PAYLOAD_INDEX_OBSERVER_V29.get() {
            observe(&mut result, budget)?;
        }
        #[cfg(test)]
        let before_base_use_check = budget.work();
        budget.charge_work(1)?;
        #[cfg(test)]
        {
            let (scan, final_check) = SOURCE_OBJECT_BASE_USE_CENSUS_WORK_V29.get();
            SOURCE_OBJECT_BASE_USE_CENSUS_WORK_V29
                .set((scan, final_check + budget.work() - before_base_use_check));
        }
        if result.occurrences.len() != event_count {
            return Err(scoped_object_error_v29());
        }
        budget.charge_work(1)?;
        #[cfg(test)]
        {
            let (classify, final_check) = SOURCE_OBJECT_READ_CENSUS_WORK_V29.get();
            SOURCE_OBJECT_READ_CENSUS_WORK_V29.set((classify, final_check + 1));
        }
        if result.reads.len() != read_count {
            return Err(scoped_object_error_v29());
        }
        #[cfg(test)]
        let before_census = budget.work();
        budget.charge_work(1)?;
        if result.projects.len() != project_count {
            return Err(scoped_object_error_v29());
        }
        #[cfg(test)]
        {
            let (calls, work) = SOURCE_OBJECT_PROJECT_CENSUS_WORK_V29.get();
            SOURCE_OBJECT_PROJECT_CENSUS_WORK_V29
                .set((calls + 1, work + budget.work() - before_census));
        }
        budget.charge_work(argument_sum_v1(&[
            argument_product_v1(result.occurrences.len(), 9)?,
            argument_product_v1(result.reads.len(), 10)?,
            argument_product_v1(result.projects.len(), 3)?,
        ])?)?;
        if result
            .occurrences
            .windows(2)
            .any(|pair| pair[0].0 >= pair[1].0)
            || result.reads.windows(2).any(|pair| pair[0].0 >= pair[1].0)
            || result
                .projects
                .windows(2)
                .any(|pair| pair[0].0 >= pair[1].0)
        {
            return Err(scoped_object_error_v29());
        }
        result.check_owner(source_index, budget)?;
        #[cfg(test)]
        source_object_record_index_work_v29(before, budget);
        #[cfg(test)]
        {
            let (calls, work) = SOURCE_OBJECT_PAYLOAD_CONSTRUCTION_WORK_V29.get();
            SOURCE_OBJECT_PAYLOAD_CONSTRUCTION_WORK_V29
                .set((calls + 1, work + budget.work() - before));
        }
        Ok(result)
    }

    fn check_owner(
        &self,
        source_index: &SourceAddressSourceIndexV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        if self.pending != source_index.pending as *const PendingScopedRootEmissionV29 as usize
            || self.ledger != budget.work_ledger_identity_v1()
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn base_occurrence(
        &self,
        source_index: &SourceAddressSourceIndexV29<'_>,
        instance: ProductionCallInstanceIdV1,
        occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ScopedMemoryOccurrenceV29, ProductionSemanticKirErrorV1> {
        #[cfg(test)]
        let before = budget.work();
        self.check_owner(source_index, budget)?;
        budget.charge_work(argument_sum_v1(&[
            9,
            argument_product_v1(9, call_splice_search_work_v1(self.occurrences.len()))?,
        ])?)?;
        let key = (
            instance.index(),
            unit_local_source_key_v1(site, role, Some(ExecutionEventV29::BaseUse)),
            place.local().index(),
        );
        let ordinal = self
            .occurrences
            .binary_search_by_key(&key, |row| row.0)
            .map_err(|_| scoped_object_error_v29())?;
        let event = self.occurrences[ordinal].1;
        let capture =
            source_object_base_occurrence_v29(occurrences, site, role, place, event, budget)?;
        #[cfg(test)]
        source_object_record_index_work_v29(before, budget);
        Ok(capture)
    }

    fn read_anchor(
        &self,
        source_index: &SourceAddressSourceIndexV29<'_>,
        source: &SourceAddressAccessSourceV29,
        anchors: &ScopedMemoryAnchorsV29,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        prefix: u32,
        occurrence: ScopedMemoryOccurrenceV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        #[cfg(test)]
        let before = budget.work();
        self.check_owner(source_index, budget)?;
        budget.charge_work(argument_sum_v1(&[
            10,
            argument_product_v1(10, call_splice_search_work_v1(self.reads.len()))?,
        ])?)?;
        let event = match occurrence {
            ScopedMemoryOccurrenceV29::Promoted { event, .. }
            | ScopedMemoryOccurrenceV29::Retained { event } => event,
        };
        let key = (
            source.instance.index(),
            unit_local_source_key_v1(site, role, Some(ExecutionEventV29::BaseUse)),
            prefix,
            event,
        );
        let ordinal = self
            .reads
            .binary_search_by_key(&key, |row| row.0)
            .map_err(|_| scoped_object_error_v29())?;
        let anchor = self.reads[ordinal].1;
        if anchor >= source.anchor {
            return Err(scoped_object_error_v29());
        }
        let row = anchors
            .rows
            .get(anchor)
            .ok_or_else(scoped_object_error_v29)?;
        let Some((_, read)) = source_object_read_payload_v29(anchors, row, budget)? else {
            return Err(scoped_object_error_v29());
        };
        if read.site != site
            || read.role != role
            || read.prefix != prefix
            || read.occurrence != occurrence
        {
            return Err(scoped_object_error_v29());
        }
        #[cfg(test)]
        source_object_record_index_work_v29(before, budget);
        Ok(anchor)
    }

    fn project_anchor(
        &self,
        source_index: &SourceAddressSourceIndexV29<'_>,
        instance: ProductionCallInstanceIdV1,
        anchors: &ScopedMemoryAnchorsV29,
        result: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        #[cfg(test)]
        let before = budget.work();
        #[cfg(test)]
        let storage_before = budget.storage();
        self.check_owner(source_index, budget)?;
        source_reference_emission_prepay_v29::<usize>(budget)?;
        source_reference_emission_prepay_v29::<Result<usize, usize>>(budget)?;
        source_reference_emission_prepay_v29::<(usize, ValueId)>(budget)?;
        source_reference_emission_prepay_v29::<Option<ValueId>>(budget)?;
        source_reference_emission_prepay_v29::<Option<&ScopedMemoryAnchorV29>>(budget)?;
        source_reference_emission_prepay_v29::<&ScopedMemoryAnchorV29>(budget)?;
        source_reference_emission_prepay_v29::<&ScopedObjectPayloadV29>(budget)?;
        budget.charge_work(argument_sum_v1(&[
            6,
            argument_product_v1(call_splice_search_work_v1(self.projects.len()), 3)?,
        ])?)?;
        let index = self
            .projects
            .binary_search_by_key(&(instance.index(), result), |row| row.0)
            .map_err(|_| scoped_object_error_v29())?;
        let anchor = self.projects[index].1;
        let row = anchors
            .rows
            .get(anchor)
            .ok_or_else(scoped_object_error_v29)?;
        let payload = anchors.object_payload(row, budget)?;
        if payload.result != Some(result)
            || !matches!(payload.operation, ScopedObjectOperationV29::Project { .. })
        {
            return Err(scoped_object_error_v29());
        }
        #[cfg(test)]
        {
            let (calls, work) = SOURCE_OBJECT_PROJECT_INDEX_WORK_V29.get();
            SOURCE_OBJECT_PROJECT_INDEX_WORK_V29.set((calls + 1, work + budget.work() - before));
        }
        #[cfg(test)]
        SOURCE_OBJECT_PROJECT_INDEX_STORAGE_V29
            .set(SOURCE_OBJECT_PROJECT_INDEX_STORAGE_V29.get() + budget.storage() - storage_before);
        #[cfg(test)]
        source_object_record_index_work_v29(before, budget);
        Ok(anchor)
    }

    fn discard(
        self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let bytes = argument_sum_v1(&[
            argument_product_v1(
                self.occurrences.capacity(),
                std::mem::size_of::<(SourceObjectOccurrenceKeyV29, usize)>(),
            )?,
            argument_product_v1(
                self.reads.capacity(),
                std::mem::size_of::<(SourceObjectReadKeyV29, usize)>(),
            )?,
            argument_product_v1(
                self.projects.capacity(),
                std::mem::size_of::<((usize, ValueId), usize)>(),
            )?,
        ])?;
        drop(self);
        budget.release_storage(bytes)?;
        Ok(())
    }
}

fn source_object_base_occurrence_v29(
    occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    place: &SemanticPlaceV1,
    event: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ScopedMemoryOccurrenceV29, ProductionSemanticKirErrorV1> {
    let row = occurrences
        .events()
        .get(event)
        .ok_or_else(scoped_object_error_v29)?;
    budget.charge_work(6)?;
    if row.site() != site
        || row.operand() != role
        || row.role() != ExecutionEventV29::BaseUse
        || row.event().variable().get() != place.local().index()
    {
        return Err(scoped_object_error_v29());
    }
    let capture = match (row.is_promoted(), row.resolved()) {
        (true, Some(SsaResolvedEventV1::Use { variable, value }))
            if variable.get() == place.local().index() =>
        {
            ScopedMemoryOccurrenceV29::Promoted {
                event,
                definition: value,
            }
        }
        (false, None) => ScopedMemoryOccurrenceV29::Retained { event },
        _ => return Err(scoped_object_error_v29()),
    };
    check_scoped_payload_occurrence_v29(occurrences, site, role, place, capture, budget)?;
    Ok(capture)
}

fn check_source_object_holder_value_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    index: &SourceAddressSourceIndexV29<'_>,
    source: &SourceAddressAccessSourceV29,
    graph: &SourceAddressMemoryV29<'_>,
    anchors: &ScopedMemoryAnchorsV29,
    payload_index: &SourceObjectPayloadIndexV29,
    endpoint: ScopedObjectEndpointV29,
    pointer: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if !matches!(endpoint.object, ScopedObjectIdentityV29::Reference { .. }) {
        return Ok(());
    }
    let ScopedObjectSourceV29::Place {
        site, role, prefix, ..
    } = endpoint.source
    else {
        return Err(scoped_object_pending_v29());
    };
    let function = instances
        .instance(source.instance)
        .ok_or_else(scoped_object_error_v29)?
        .declaration();
    let place = scoped_object_original_place_v29(function, site, role)
        .ok_or_else(scoped_object_error_v29)?;
    source_reference_owned_prepay_v29::<Option<&SemanticTypeDeclV1>>(plan, budget)?;
    source_reference_owned_prepay_v29::<Option<&SemanticTypeShapeV1>>(plan, budget)?;
    budget.source_reference_charge_v29(plan, 2)?;
    let sidecar = index.sidecar(source.instance, budget)?;
    let archive = sidecar
        .execution_observation
        .as_ref()
        .ok_or_else(execution_archive_error_v29)?;
    let occurrences = instances
        .occurrences(source.instance)
        .ok_or_else(execution_archive_error_v29)?;
    let capture = payload_index.base_occurrence(
        index,
        source.instance,
        &occurrences,
        site,
        role,
        place,
        budget,
    )?;
    if matches!(
        instances
            .owner()
            .source_semantic()
            .types()
            .get(endpoint.root_type.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Tuple(_) | SemanticTypeShapeV1::Aggregate(_))
    ) {
        return check_source_aggregate_object_value_v29(
            instances,
            plan,
            index,
            source.instance,
            graph,
            anchors,
            payload_index,
            endpoint,
            pointer,
            capture,
            budget,
        );
    }
    if prefix != 0
        && prefix as usize == place.projections().len()
        && place.projections()[prefix as usize - 1].kind() == SemanticProjectionKindV1::Dereference
        && matches!(instances.owner().source_semantic().types()[if prefix == 1 {
            function.locals()[place.local().index() as usize].ty().index() as usize
        } else { place.projections()[prefix as usize - 2].result_type().index() as usize }].shape(),
            SemanticTypeShapeV1::Pointer(pointer) if pointer.kind() == SemanticPointerKindV1::Reference
                && pointer.metadata() == SemanticPointerMetadataV1::None)
    {
        let (block, statement) = scoped_memory_site_key_v29(site);
        let original_site = SourceReferenceSiteV29 {
            instance: source.instance,
            block: SemanticBlockIdV1::from_index(block),
            statement: statement.map(|value| value as usize),
        };
        let access =
            source_reference_raw_original_access_v29(function, original_site, place, budget)?
                .ok_or_else(scoped_object_error_v29)?;
        let loan = source_object_loan_access_v29(plan, original_site, place, access, budget)?
            .ok_or_else(scoped_object_error_v29)?;
        // A retained safe holder cannot use a raw-holder read receipt.
        let ScopedMemoryOccurrenceV29::Promoted { definition, .. } = capture else {
            return Err(scoped_object_pending_v29());
        };
        let binding =
            archive.lookup_original_v29(instances, source.instance, definition, budget)?;
        let selected = source_object_loan_holder_v29(plan, binding, place, &loan, budget)?;
        if selected.values[0].id != pointer {
            return Err(scoped_object_error_v29());
        }
        return Ok(());
    }
    if prefix == 0
        || prefix as usize != place.projections().len()
        || place.projections()[prefix as usize - 1].kind() != SemanticProjectionKindV1::Dereference
    {
        return Err(scoped_object_pending_v29());
    }
    let holder_prefix = prefix - 1;
    if holder_prefix != 0 {
        let (block, statement) = scoped_memory_site_key_v29(site);
        let original_site = SourceReferenceSiteV29 {
            instance: source.instance,
            block: SemanticBlockIdV1::from_index(block),
            statement: statement.map(|value| value as usize),
        };
        let access =
            source_reference_raw_original_access_v29(function, original_site, place, budget)?
                .ok_or_else(scoped_object_error_v29)?;
        source_static_raw_holder_v42(
            plan,
            original_site,
            place,
            access,
            holder_prefix as usize,
            budget,
        )?;
        // Nested holders are stored pointer cells. An SSA value for the whole
        // aggregate is not an authenticated pointer load from that cell.
        if !matches!(capture, ScopedMemoryOccurrenceV29::Retained { .. }) {
            return Err(scoped_object_pending_v29());
        }
    }
    if let ScopedMemoryOccurrenceV29::Promoted { definition, .. } = capture {
        let binding =
            archive.lookup_original_v29(instances, source.instance, definition, budget)?;
        if !matches!(binding, SemanticValueBindingV1::Value { id, .. } if *id == pointer) {
            return Err(scoped_object_error_v29());
        }
    } else {
        let ordinal = payload_index.read_anchor(
            index,
            source,
            anchors,
            site,
            role,
            holder_prefix,
            capture,
            budget,
        )?;
        let row = anchors
            .rows
            .get(ordinal)
            .ok_or_else(scoped_object_error_v29)?;
        budget.charge_work(4)?;
        let Some((_, ScopedMemoryPayloadV29::Load { result, read })) =
            source_address_object_payload_v29(anchors, row, budget)?
        else {
            return Err(scoped_object_error_v29());
        };
        if read.site == site
            && read.role == role
            && read.prefix == holder_prefix
            && read.occurrence == capture
        {
            let actual = source_address_original_operation_v29(
                index.pending,
                graph,
                source.instance,
                row.block,
                row.position,
                budget,
            )?;
            if result != pointer
                || !matches!(actual.results.as_slice(), [value] if value.id == pointer)
                || ordinal >= source.anchor
            {
                return Err(scoped_object_error_v29());
            }
        } else {
            return Err(scoped_object_error_v29());
        }
    }
    Ok(())
}

fn check_source_object_stored_read_v29(
    index: &SourceAddressSourceIndexV29<'_>,
    source: &SourceAddressAccessSourceV29,
    graph: &SourceAddressMemoryV29<'_>,
    anchors: &ScopedMemoryAnchorsV29,
    payload_index: &SourceObjectPayloadIndexV29,
    place: &SemanticPlaceV1,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    occurrence: ScopedMemoryOccurrenceV29,
    value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let prefix =
        u32::try_from(place.projections().len()).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    let anchor = payload_index.read_anchor(
        index, source, anchors, site, role, prefix, occurrence, budget,
    )?;
    let row = anchors
        .rows
        .get(anchor)
        .ok_or_else(scoped_object_error_v29)?;
    budget.charge_work(4)?;
    let Some((result, read)) = source_object_read_payload_v29(anchors, row, budget)? else {
        return Err(scoped_object_error_v29());
    };
    if read.site == site
        && read.role == role
        && read.prefix as usize == place.projections().len()
        && read.ty == place.ty()
        && read.occurrence == occurrence
    {
        let actual = source_address_original_operation_v29(
            index.pending,
            graph,
            source.instance,
            row.block,
            row.position,
            budget,
        )?;
        if result != value || !matches!(actual.results.as_slice(), [result] if result.id == value) {
            return Err(scoped_object_error_v29());
        }
    } else {
        return Err(scoped_object_error_v29());
    }
    Ok(())
}

// A stored scalar can come from either retained physical representation. This
// supplies only a read locator; its actual operation and source occurrence are
// rejoined below, and complete final memory admission still proves readability.
fn source_object_read_payload_v29(
    anchors: &ScopedMemoryAnchorsV29,
    row: &ScopedMemoryAnchorV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<(ValueId, ScopedMemoryReadV29)>, ProductionSemanticKirErrorV1> {
    if let ScopedMemoryAnchorKindV29::Access {
        payload: Some(ScopedMemoryPayloadV29::Load { result, read }),
        ..
    } = row.kind
    {
        budget.charge_work(1)?;
        return Ok(Some((result, read)));
    }
    Ok(
        match source_address_object_payload_v29(anchors, row, budget)? {
            Some((_, ScopedMemoryPayloadV29::Load { result, read })) => Some((result, read)),
            _ => None,
        },
    )
}
