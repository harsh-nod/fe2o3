// Tag-only accesses retain their own original occurrence and logical lifetime.
// They never borrow a fabricated scalar payload from the value-access adapter.
fn source_address_tag_query_headers_v43() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> usize {
        std::mem::size_of::<T>()
            + 2 * std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
    }
    argument_sum_v1(&[
        h::<SourceAddressAccessSourceV29>(),
        h::<Option<SourceAddressAccessSourceV29>>(),
        h::<ScopedObjectEndpointV29>(),
        h::<ScopedObjectTagOriginV29>(),
        h::<SourceDirectObjectOriginV29>(),
        h::<SourceAddressTagAccessV43>(),
        h::<SourceTagDemandV43>(),
        h::<Option<(usize, SourceTagDemandV43)>>(),
        h::<SourceReferenceAccessKeyV29>(),
        h::<SourceReferenceAccessIndexKeyV29>(),
        h::<SourceReferenceSiteV29>(),
        h::<ScopedMemoryFrameV29>(),
        h::<ProductionSemanticSsaFunctionOccurrencesV1<'_>>(),
        h::<ScopedEmittedPointsV29<'_, '_, '_>>(),
        h::<SourceStaticObjectLocationV29>(),
        h::<Option<(u64, u64)>>(),
        20 * std::mem::size_of::<usize>(),
        16 * std::mem::size_of::<&()>(),
    ])
}

// The caller prepays the fixed query envelope once for its bounded census.
fn source_address_tag_source_v43(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    instance: ProductionCallInstanceIdV1,
    anchor: usize,
    recorded: &ScopedMemoryAnchorsV29,
    row: &ScopedMemoryAnchorV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceAddressAccessSourceV29>, ProductionSemanticKirErrorV1> {
    if !matches!(row.kind, ScopedMemoryAnchorKindV29::Object(_)) {
        return Ok(None);
    }
    let payload = recorded.object_payload(row, budget)?;
    let (endpoint, origin, writing) = match (payload.operation, payload.role) {
        (
            ScopedObjectOperationV29::ReadDiscriminant { .. },
            ScopedObjectRoleV29::ReadDiscriminant { source, origin },
        ) => (source, origin, false),
        (
            ScopedObjectOperationV29::SetDiscriminant { variant, .. },
            ScopedObjectRoleV29::SetDiscriminant {
                destination,
                origin,
                variant: original,
            },
        ) if variant == original => (destination, origin, true),
        (
            ScopedObjectOperationV29::ReadDiscriminant { .. }
            | ScopedObjectOperationV29::SetDiscriminant { .. },
            _,
        ) => {
            return Err(scoped_object_error_v29());
        }
        _ => return Ok(None),
    };
    budget.charge_work(9)?;
    let site = match origin {
        ScopedObjectTagOriginV29::Statement(site) => site,
        ScopedObjectTagOriginV29::Aggregate(site) if writing => site,
        _ => return Err(scoped_object_pending_v29()),
    };
    let ScopedObjectIdentityV29::Local {
        instance: owner,
        local,
        generation,
    } = endpoint.object
    else {
        return Err(scoped_object_pending_v29());
    };
    if owner != instance {
        return Err(scoped_object_error_v29());
    }
    let original = instances
        .instance(instance)
        .ok_or_else(scoped_object_error_v29)?;
    let occurrences = instances
        .occurrences(instance)
        .ok_or_else(scoped_object_error_v29)?;
    recorded.check_object_source(
        original.declaration(),
        &occurrences,
        anchor,
        row,
        payload,
        budget,
    )?;
    let frame = row.source.ok_or_else(scoped_object_error_v29)?;
    if frame.site != site {
        return Err(scoped_object_error_v29());
    }
    source_index.frame_gap(instance, frame, row.block, row.position, budget)?;
    let target = source_address_object_slot_v29(
        instances,
        plan,
        slots,
        instance,
        local,
        generation,
        endpoint.root_type,
        budget,
    )?;
    let mut mapping = ScopedEmittedPointsV29 {
        coordinates: &source_index.pending.coordinates,
        relocation: &source_index.pending.slot_relocation,
        budget,
    };
    let (block, operation) = mapping
        .emitted_point(
            instance,
            row.block,
            u32::try_from(row.position).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            false,
        )
        .map_err(source_address_point_error_v29)?
        .ok_or_else(scoped_object_error_v29)?;
    Ok(Some(SourceAddressAccessSourceV29 {
        instance,
        anchor,
        physical: SourceAddressAccessV29 {
            block,
            operation: operation as usize,
            footprint: 0,
            slot: target,
        },
        raw: None,
        direct_object: Some(SourceDirectObjectOriginV29 {
            instance,
            local,
            generation,
            ty: endpoint.root_type,
        }),
        safe_object: None,
    }))
}

fn check_source_address_tag_payload_v43(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    source_index: &SourceAddressSourceIndexV29<'_>,
    graph: &SourceAddressMemoryV29<'_>,
    source: &SourceAddressAccessSourceV29,
    recorded: &ScopedMemoryAnchorsV29,
    row: &ScopedMemoryAnchorV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let Some(expected) = source_address_tag_source_v43(
        instances,
        references.plan,
        source_index,
        slots,
        source.instance,
        source.anchor,
        recorded,
        row,
        budget,
    )?
    else {
        return Ok(false);
    };
    budget.charge_work(8)?;
    if source.physical != expected.physical
        || source.direct_object != expected.direct_object
        || source.raw.is_some()
        || source.safe_object.is_some()
    {
        return Err(scoped_object_error_v29());
    }
    let payload = recorded.object_payload(row, budget)?;
    let endpoint = match payload.role {
        ScopedObjectRoleV29::ReadDiscriminant { source, .. } => source,
        ScopedObjectRoleV29::SetDiscriminant { destination, .. } => destination,
        _ => return Err(scoped_object_error_v29()),
    };
    let operation = graph.blocks[graph.block(source.physical.block, budget)?]
        .1
        .operations
        .get(source.physical.operation)
        .ok_or_else(scoped_object_error_v29)?;
    payload.check_operation(operation, budget)?;
    let access = source_address_tag_access_v43(operation)?.ok_or_else(scoped_object_error_v29)?;
    if graph.exact(access.pointer, budget)? != Some(source.physical.slot) {
        return Err(scoped_object_error_v29());
    }
    let expected = source_static_object_expected_location_v29(
        instances,
        references.plan,
        source_index,
        slots,
        endpoint,
        budget,
    )?
    .ok_or_else(scoped_object_pending_v29)?;
    if graph.object_location(access.pointer, budget)? != expected {
        return Err(scoped_object_error_v29());
    }
    // None is an authenticated no-byte-effect operation. It remains in the
    // access/currentness census; it does not initialize or invalidate a byte.
    graph.object_tag_range_v43(
        access,
        slots
            .slots
            .get(source.physical.slot)
            .ok_or_else(scoped_object_error_v29)?,
        budget,
    )?;
    Ok(true)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceTagDemandV43 {
    Statement,
    Aggregate,
}

fn source_address_original_tag_demand_v43(
    function: &SemanticFunctionDeclV1,
    key: SourceReferenceAccessKeyV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceTagDemandV43>, ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    let Some(statement) = key.site.statement else {
        return Ok(None);
    };
    let statement = function
        .blocks()
        .get(key.site.block.index() as usize)
        .and_then(|block| block.statements().get(statement))
        .ok_or_else(scoped_object_error_v29)?;
    let (place, access, demand) = match statement.kind() {
        SemanticStatementKindV1::SetDiscriminant { place, .. } => (
            place,
            SourceReferenceAccessV29::Write,
            SourceTagDemandV43::Statement,
        ),
        SemanticStatementKindV1::Assign(assignment) => match assignment.value().kind() {
            SemanticRvalueKindV1::Discriminant(place) => (
                place,
                SourceReferenceAccessV29::ReadDiscriminant,
                SourceTagDemandV43::Statement,
            ),
            SemanticRvalueKindV1::Aggregate(aggregate)
                if matches!(aggregate.kind(), SemanticAggregateKindV1::EnumVariant(_)) =>
            {
                (
                    assignment.destination(),
                    SourceReferenceAccessV29::Write,
                    SourceTagDemandV43::Aggregate,
                )
            }
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };
    Ok(
        (key.source == place as *const SemanticPlaceV1 as usize && key.access == access)
            .then_some(demand),
    )
}

fn source_address_tag_effect_v43(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    source: &SourceAddressAccessSourceV29,
    recorded: &ScopedMemoryAnchorsV29,
    row: &ScopedMemoryAnchorV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<(usize, SourceTagDemandV43)>, ProductionSemanticKirErrorV1> {
    let Some(expected) = source_address_tag_source_v43(
        instances,
        plan,
        source_index,
        slots,
        source.instance,
        source.anchor,
        recorded,
        row,
        budget,
    )?
    else {
        return Ok(None);
    };
    budget.charge_work(8)?;
    if expected.physical != source.physical
        || expected.direct_object != source.direct_object
        || source.raw.is_some()
        || source.safe_object.is_some()
    {
        return Err(scoped_object_error_v29());
    }
    let payload = recorded.object_payload(row, budget)?;
    let (endpoint, origin, access) = match payload.role {
        ScopedObjectRoleV29::ReadDiscriminant { source, origin } => {
            (source, origin, SourceReferenceAccessV29::ReadDiscriminant)
        }
        ScopedObjectRoleV29::SetDiscriminant {
            destination,
            origin,
            ..
        } => (destination, origin, SourceReferenceAccessV29::Write),
        _ => return Err(scoped_object_error_v29()),
    };
    let ScopedObjectSourceV29::Place {
        site,
        role,
        local,
        prefix,
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
    if place.local() != local || prefix as usize != place.projections().len() {
        return Err(scoped_object_error_v29());
    }
    let (block, statement) = scoped_memory_site_key_v29(site);
    let site = SourceReferenceSiteV29 {
        instance: source.instance,
        block: SemanticBlockIdV1::from_index(block),
        statement: statement.map(|value| value as usize),
    };
    let key = SourceReferenceAccessKeyV29 {
        site,
        source: place as *const SemanticPlaceV1 as usize,
        access,
    };
    let index_key = source_reference_access_key_v29(site, place, access);
    charge_execution_cfg_lookup_v29(plan.access_sites.len(), budget)?;
    let index = *plan
        .access_sites
        .get(&index_key)
        .ok_or_else(scoped_object_error_v29)?;
    let record = plan
        .accesses
        .get(index)
        .ok_or_else(scoped_object_error_v29)?;
    let direct = expected.direct_object.ok_or_else(scoped_object_error_v29)?;
    let demand = source_address_original_tag_demand_v43(function, key, budget)?
        .ok_or_else(scoped_object_error_v29)?;
    if record.key != key
        || record.instance != direct.instance
        || record.local != direct.local
        || record.generation != direct.generation
        || record.ty != endpoint.projected_type
        || demand
            != match origin {
                ScopedObjectTagOriginV29::Statement(_) => SourceTagDemandV43::Statement,
                ScopedObjectTagOriginV29::Aggregate(_) => SourceTagDemandV43::Aggregate,
                _ => return Err(scoped_object_pending_v29()),
            }
    {
        return Err(scoped_object_error_v29());
    }
    Ok(Some((index, demand)))
}
