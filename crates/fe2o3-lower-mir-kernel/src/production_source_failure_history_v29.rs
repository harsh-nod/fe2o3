// Diagnostic reads have no physical load. Their original read and optional
// move are checked in a failure-only shadow, never in the successful CFG state.
include!("production_source_scalar_ranges_v45.rs");
type SourceFailureScalarPathFrameV43<'a> = (
    &'a SemanticFunctionDeclV1,
    &'a [SemanticTypeDeclV1],
    &'a SemanticPlaceV1,
    SemanticTypeIdV1,
    std::slice::Iter<'a, SemanticProjectionV1>,
    &'a SemanticProjectionV1,
    &'a SemanticTypeDeclV1,
    Option<SemanticTypeIdV1>,
);

fn source_failure_scalar_path_v43(
    original: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    place: &SemanticPlaceV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    with_canonical_call_scratch_v1(budget, |budget| {
        budget.reserve_storage(source_reference_emission_headers_v29::<
            SourceFailureScalarPathFrameV43<'_>,
        >()?)?;
        budget.charge_work(3)?;
        let mut ty = original
            .locals()
            .get(place.local().index() as usize)
            .ok_or_else(scoped_memory_error_v29)?
            .ty();
        for projection in place.projections() {
            budget.charge_work(5)?;
            let declaration = types
                .get(ty.index() as usize)
                .ok_or_else(scoped_memory_error_v29)?;
            ty = match (declaration.shape(), projection.kind()) {
                (
                    SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
                    SemanticProjectionKindV1::Field(field),
                ) => *fields
                    .fields()
                    .get(field as usize)
                    .ok_or_else(scoped_memory_error_v29)?,
                (
                    SemanticTypeShapeV1::Array { element, length },
                    SemanticProjectionKindV1::ConstantIndex {
                        offset,
                        minimum_length,
                        from_end,
                    },
                ) => {
                    source_static_constant_index_v29(*length, offset, minimum_length, from_end)?;
                    *element
                }
                // These paths can observe a pointer, tag, or dynamic selector.
                // A promoted base alone does not make that observation scalar SSA.
                _ => return Ok(false),
            };
            if ty != projection.result_type() {
                return Err(scoped_memory_error_v29());
            }
        }
        budget.charge_work(2)?;
        if ty != place.ty() {
            return Err(scoped_memory_error_v29());
        }
        Ok(matches!(
            types
                .get(ty.index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
        ))
    })
}

fn source_failure_operand_moved_v29(
    original: &SemanticFunctionDeclV1,
    row: &ScopedMemoryAnchorV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    let frame = row.source.ok_or_else(scoped_memory_error_v29)?;
    let Some(ScopedMemoryRoleV29::Operand(role @ ExecutionOperandV29::AssertMessage(_))) =
        frame.role
    else {
        return Err(scoped_memory_error_v29());
    };
    match scoped_source_operand_v29(original, frame.site, role) {
        Some(SemanticOperandV1::Copy(_)) => Ok(false),
        Some(SemanticOperandV1::Move(_)) => Ok(true),
        _ => Err(scoped_memory_error_v29()),
    }
}

fn source_failure_scalar_slot_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    site: SourceReferenceSiteV29,
    place: &SemanticPlaceV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(usize, SourceScalarByteRangeV45), ProductionSemanticKirErrorV1> {
    budget.charge_work(8)?;
    if slots.ledger != budget.work_ledger_identity_v1()
        || slots.source != ExecutionCallSourceV29::from_instances(instances, budget)?
    {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let original = instances
        .instance(site.instance)
        .ok_or_else(source_raw_physical_error_v29)?
        .declaration();
    let (root_type, range) = source_scalar_range_v45(
        original,
        instances.owner().source_semantic().types(),
        place,
        budget,
    )?;
    let access =
        source_reference_access_at_v29(plan, site, place, SourceReferenceAccessV29::Read, budget)?;
    budget.charge_work(place.projections().len())?;
    if access.instance != site.instance
        || access.local != place.local()
        || access.ty != place.ty()
        || access.loan.is_some()
        || plan.projections.get(access.projections.clone()) != Some(place.projections())
        || !access.traversed.is_empty()
    {
        return Err(source_raw_physical_error_v29());
    }
    budget.charge_work(call_splice_search_work_v1(slots.instances.len()))?;
    let owner = slots
        .instances
        .binary_search_by_key(&site.instance.index(), |row| row.instance.index())
        .ok()
        .and_then(|index| slots.instances.get(index))
        .ok_or_else(source_raw_physical_error_v29)?;
    if instances.instance(site.instance).map(|row| row.function()) != Some(owner.function) {
        return Err(source_raw_physical_error_v29());
    }
    let rows = slots
        .slots
        .get(owner.slots.clone())
        .ok_or_else(source_raw_physical_error_v29)?;
    let (legacy, objects) =
        source_address_local_slot_ranges_v29(rows, place.local().index(), budget)?;
    let index = match (legacy, objects.is_empty()) {
        (Some(index), true) => index,
        (None, false) => {
            let slot = source_address_object_slot_v29(
                instances,
                plan,
                slots,
                site.instance,
                place.local(),
                access.generation,
                root_type,
                budget,
            )?;
            return Ok((slot, range));
        }
        _ => return Err(source_raw_physical_error_v29()),
    };
    let slot = &rows[index];
    if slot.instance != site.instance
        || !place.projections().is_empty()
        || slot.origin.semantic_type != place.ty()
        || matches!(slot.representation, ScopedSlotRepresentationV29::ScalarArray(scalar)
            if scalar.length != 1 || scalar.element_type != place.ty()
                || !matches!(scalar.element.element, PrivateRetainedElementFactsV1::Scalar(_)))
    {
        return Err(source_raw_physical_error_v29());
    }
    Ok((argument_sum_v1(&[owner.slots.start, index])?, range))
}

fn source_boundary_is_failure_move_v29(row: &SourceAddressBoundaryV29) -> bool {
    row.cause == ScopedMemoryKillV29::Move
        && matches!(
            row.frame.role,
            Some(ScopedMemoryRoleV29::Operand(
                ExecutionOperandV29::AssertMessage(_)
            ))
        )
}
