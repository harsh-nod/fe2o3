// A checked original element read permits emission, not an initialization fact
// for the whole slot. Final sparse history still proves the actual read.
fn source_partial_array_read_headers_v29() -> Result<usize, ArgumentResourceV1> {
    use std::mem::size_of;
    argument_sum_v1(&[
        size_of::<Option<SourceFunctionBackingViewV29<'_>>>(),
        size_of::<Result<Option<SourceFunctionBackingViewV29<'_>>, ProductionSemanticKirErrorV1>>(),
        size_of::<Option<(ScopedMemoryReadV29, bool)>>(),
        size_of::<Result<Option<&SemanticRetainedLocalSlotV1>, ProductionSemanticKirErrorV1>>(),
        size_of::<Result<&SourceReferenceAccessRecordV29, ProductionSemanticKirErrorV1>>(),
        size_of::<Option<(usize, SourceReferenceScalarCellV29)>>(),
        size_of::<Result<Option<(usize, SourceReferenceScalarCellV29)>, ProductionSemanticKirErrorV1>>(),
        size_of::<Option<PrivateRetainedArrayFactsV1>>(),
        size_of::<Result<Option<PrivateRetainedArrayFactsV1>, ProductionSemanticKirErrorV1>>(),
        size_of::<Result<(&Type, u32, Option<SemanticRetainedArrayLayoutV1>), ProductionSemanticKirErrorV1>>(),
        size_of::<[usize; 16]>(),
    ])
}

fn check_source_partial_array_read_v29(
    backing: SourceFunctionBackingViewV29<'_>,
    read: ScopedMemoryReadV29,
    place: &SemanticPlaceV1,
    slot: &SemanticRetainedLocalSlotV1,
    mut budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    backing.layouts.check(budget)?;
    let plan = backing.layouts.references;
    budget.source_reference_charge_v29(plan, 18)?;
    let original = plan.instances.instance(backing.instance)
        .ok_or_else(scoped_object_allocation_error_v29)?.declaration();
    if plan.storage_root.is_none()
        || read.ty != place.ty() || read.prefix as usize != place.projections().len()
        || !matches!(read.occurrence, ScopedMemoryOccurrenceV29::Retained { .. })
        || !scoped_payload_place_v29(original, read.site, read.role)
            .is_some_and(|source| std::ptr::eq(source, place))
    {
        return Err(scoped_object_allocation_error_v29());
    }
    let [projection] = place.projections() else {
        return Err(scoped_object_allocation_error_v29());
    };
    let SemanticProjectionKindV1::ConstantIndex { offset, minimum_length, from_end } = projection.kind() else {
        return Err(scoped_object_allocation_error_v29());
    };
    let (block, statement) = scoped_memory_site_key_v29(read.site);
    let site = SourceReferenceSiteV29 {
        instance: backing.instance,
        block: SemanticBlockIdV1::from_index(block),
        statement: statement.map(|statement| statement as usize),
    };
    let access = source_reference_access_at_v29(plan, site, place, SourceReferenceAccessV29::Read, budget)?;
    budget.source_reference_charge_v29(plan, 13)?;
    if access.instance != backing.instance || access.local != place.local()
        || access.loan.is_some() || !access.traversed.is_empty() || access.shared_path
        || plan.projections.get(access.projections.clone()) != Some(place.projections())
        || original.locals().get(place.local().index() as usize).map(|local| local.ty()) != Some(slot.semantic_type)
    {
        return Err(scoped_object_allocation_error_v29());
    }
    let (cell, row) = backing.cell(access.local, access.generation, budget)?
        .ok_or_else(scoped_object_allocation_error_v29)?;
    charge_execution_cfg_lookup_v29(backing.layouts.backing.len(), budget)?;
    let selected = backing.layouts.backing.get(&(backing.instance.index(), place.local().index(), access.generation))
        .ok_or_else(scoped_object_allocation_error_v29)?;
    budget.source_reference_charge_v29(plan, 5)?;
    if selected.cell != cell || selected.array.is_none()
        || selected.array.map(SourceBackingKindV29::Object) != Some(row.kind)
        || row.ty != slot.semantic_type
    {
        return Err(scoped_object_allocation_error_v29());
    }
    let facts = private_retained_array_facts_v1(
        plan.instances.owner().source_semantic().types(), slot.semantic_type, usize::MAX, &mut budget,
    )?.ok_or_else(scoped_object_allocation_error_v29)?;
    let PrivateRetainedElementFactsV1::Scalar(scalar) = facts.element.element else {
        return Err(scoped_object_allocation_error_v29());
    };
    let (actual, alignment, array) = slot.storage.scalar_array()?;
    let array = array.ok_or_else(scoped_object_allocation_error_v29)?;
    budget.source_reference_charge_v29(plan, 12)?;
    let index = if from_end { facts.length.checked_sub(offset) } else { Some(offset) };
    if facts.element_type != place.ty() || projection.result_type() != place.ty()
        || array.element != facts.element_type || array.length != facts.length
        || *actual != Type::Scalar(scalar) || alignment != facts.element.alignment
        || minimum_length > facts.length || index.is_none_or(|index| index >= facts.length)
    {
        return Err(scoped_object_allocation_error_v29());
    }
    Ok(())
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn require_source_partial_array_read_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.scoped_memory.is_none()
            || !matches!(place.projections(), [projection]
                if matches!(projection.kind(), SemanticProjectionKindV1::ConstantIndex { .. }))
        {
            return self.require_retained_array_initialized_v1(block, statement, place.local());
        }
        self.with_scoped_payload_header_v29(source_partial_array_read_headers_v29()?, |this| {
            this.with_emission_budget_v1(|this, budget| {
                let backing = SourceFunctionBackingViewV29::new(
                    &this.defined_function_signatures, this.execution.as_ref(), this.semantic_function, budget,
                )?.ok_or_else(scoped_object_allocation_error_v29)?;
                budget.source_reference_charge_v29(backing.layouts.references, 4)?;
                let (read, _) = this.scoped_memory.as_ref().and_then(|recorder| recorder.read_payload)
                    .ok_or_else(scoped_object_allocation_error_v29)?;
                if read.site != execution_site_v29(block, statement) {
                    return Err(scoped_object_allocation_error_v29());
                }
                let slot = lookup_optional_retained_array_v29(
                    &this.retained_local_slots, place.local().index(), Some(budget),
                )?.ok_or_else(scoped_object_allocation_error_v29)?;
                check_source_partial_array_read_v29(backing, read, place, slot, budget)
            })
        })
    }
}
